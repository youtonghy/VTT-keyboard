use crate::processing;
use crate::recorder::RecordedAudio;
use crate::sensevoice::ensure_service_ready_blocking;
use crate::settings::{
    ProcessingFailureStage, SettingsStore, TranscriptionAlignment, TranscriptionHistoryItem,
    TranscriptionHistoryStatus,
};
use crate::status_native::StatusActionSet;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// 本地 SenseVoice 服务（包括 vLLM）冷启动最长等待时间。
/// vLLM 模型装载可能耗时数分钟，给足 8 分钟的上限以便在系统重启后自动恢复。
const SENSEVOICE_READY_TIMEOUT: Duration = Duration::from_secs(8 * 60);
const RETRY_SOURCE_CACHE_LIMIT: usize = 20;

enum DispatchMessage {
    Process(RecordedAudio),
    RetryActive,
    RetryHistory {
        history_id: String,
        reply: mpsc::Sender<Result<TranscriptionHistoryItem, String>>,
    },
    CancelActive,
    Shutdown,
}

#[derive(Clone)]
enum RetrySource {
    Recording(RecordedAudio),
    TranscribedText {
        text: String,
        model_group: String,
        transcription_elapsed_ms: u64,
        recording_duration_ms: u64,
        alignment: Option<TranscriptionAlignment>,
    },
}

#[derive(Clone)]
struct HistoryReference {
    id: String,
    timestamp_ms: u64,
}

#[derive(Clone)]
struct ActiveRetry {
    source: RetrySource,
    attempts: u32,
    history: Option<HistoryReference>,
}

pub struct TranscriptionDispatcher {
    sender: mpsc::Sender<DispatchMessage>,
    worker: Option<thread::JoinHandle<()>>,
}

impl TranscriptionDispatcher {
    pub fn new(app: AppHandle, store: SettingsStore) -> Self {
        let (sender, receiver) = mpsc::channel::<DispatchMessage>();
        let dispatcher_app = app.clone();
        let worker = thread::spawn(move || {
            let mut active_retry: Option<ActiveRetry> = None;
            let mut retry_sources: HashMap<String, RetrySource> = HashMap::new();
            let mut retry_source_order: VecDeque<String> = VecDeque::new();

            while let Ok(message) = receiver.recv() {
                match message {
                    DispatchMessage::Process(recording) => {
                        active_retry = None;
                        let recording_for_retry = recording.clone();
                        // 在真正调用转写引擎前，若当前使用的是 SenseVoice 本地服务，
                        // 自动检查 Docker 容器/原生模型状态并按需创建/恢复/启动。
                        // 这样可以在系统重启等情况下自动恢复容器，无需用户手动点击"启动服务"。
                        if let Err(err) = ensure_sensevoice_runtime_ready(&dispatcher_app, &store) {
                            #[cfg(debug_assertions)]
                            eprintln!("SenseVoice 运行时自动恢复失败: {err}");
                        }
                        let outcome = processing::handle_recording(&store, recording);
                        if !outcome.is_success() {
                            #[cfg(debug_assertions)]
                            {
                                if let Some(error_message) = outcome.error_message.as_ref() {
                                    eprintln!("录音处理失败: {error_message}");
                                }
                            }
                        }

                        let mut history_ref = None;
                        if outcome.history_enabled {
                            let item = history_item_from_outcome(
                                create_history_id(),
                                now_timestamp_ms(),
                                outcome.clone(),
                            );
                            history_ref = Some(HistoryReference {
                                id: item.id.clone(),
                                timestamp_ms: item.timestamp_ms,
                            });
                            if let Err(_err) = store.append_transcription_history(item.clone()) {
                                #[cfg(debug_assertions)]
                                eprintln!("写入历史记录失败: {_err}");
                                history_ref = None;
                            } else if let Err(_err) =
                                app.emit("transcription-history-appended", &item)
                            {
                                #[cfg(debug_assertions)]
                                eprintln!("发送历史记录事件失败: {_err}");
                            }
                        }

                        if !outcome.is_success() {
                            let error_text = outcome_error_text(&outcome);
                            if outcome.is_retryable_failure() {
                                if let Some(source) = retry_source_from_recording_outcome(
                                    &recording_for_retry,
                                    &outcome,
                                ) {
                                    if let Some(history) = history_ref.as_ref() {
                                        remember_retry_source(
                                            &mut retry_sources,
                                            &mut retry_source_order,
                                            history.id.clone(),
                                            source.clone(),
                                        );
                                    }
                                    active_retry = Some(ActiveRetry {
                                        source,
                                        attempts: 1,
                                        history: history_ref,
                                    });
                                    processing::emit_retryable_failure_status(
                                        error_text,
                                        StatusActionSet::Retry,
                                    );
                                    continue;
                                }
                            }
                            processing::emit_status_detail(
                                "error",
                                &processing::failure_status(error_text),
                            );
                        }
                    }
                    DispatchMessage::RetryActive => {
                        let Some(current) = active_retry.clone() else {
                            continue;
                        };
                        processing::emit_status("transcribing");
                        let attempts = current.attempts.saturating_add(1);
                        let outcome = process_retry_source(
                            &dispatcher_app,
                            &store,
                            current.source.clone(),
                            processing::ProcessingOptions::default(),
                        );
                        if let Some(history) = current.history.as_ref() {
                            let item = history_item_from_outcome(
                                history.id.clone(),
                                history.timestamp_ms,
                                outcome.clone(),
                            );
                            replace_history_item(&app, &store, item);
                        }

                        if outcome.is_success() {
                            if let Some(history) = current.history.as_ref() {
                                retry_sources.remove(&history.id);
                                retry_source_order.retain(|history_id| history_id != &history.id);
                            }
                            active_retry = None;
                            continue;
                        }

                        let error_text = outcome_error_text(&outcome);
                        if outcome.is_retryable_failure() {
                            if let Some(next_source) =
                                retry_source_from_previous_outcome(&current.source, &outcome)
                            {
                                if let Some(history) = current.history.as_ref() {
                                    remember_retry_source(
                                        &mut retry_sources,
                                        &mut retry_source_order,
                                        history.id.clone(),
                                        next_source.clone(),
                                    );
                                }
                                active_retry = Some(ActiveRetry {
                                    source: next_source,
                                    attempts,
                                    history: current.history,
                                });
                                let actions = if attempts >= 2 {
                                    StatusActionSet::RetryCancel
                                } else {
                                    StatusActionSet::Retry
                                };
                                processing::emit_retryable_failure_status(error_text, actions);
                                continue;
                            }
                        }

                        active_retry = None;
                        processing::emit_status_detail(
                            "error",
                            &processing::failure_status(error_text),
                        );
                    }
                    DispatchMessage::RetryHistory { history_id, reply } => {
                        let result = retry_history_item(
                            &dispatcher_app,
                            &store,
                            &mut retry_sources,
                            &mut retry_source_order,
                            &history_id,
                        );
                        let _ = reply.send(result);
                    }
                    DispatchMessage::CancelActive => {
                        active_retry = None;
                        crate::status_native::hide();
                    }
                    DispatchMessage::Shutdown => break,
                }
            }
        });

        Self {
            sender,
            worker: Some(worker),
        }
    }

    pub fn enqueue(&self, recording: RecordedAudio) -> Result<(), String> {
        self.sender
            .send(DispatchMessage::Process(recording))
            .map_err(|_| "转写任务线程不可用".to_string())
    }

    pub fn retry_active(&self) -> Result<(), String> {
        self.sender
            .send(DispatchMessage::RetryActive)
            .map_err(|_| "转写任务线程不可用".to_string())
    }

    pub fn cancel_active_retry(&self) -> Result<(), String> {
        self.sender
            .send(DispatchMessage::CancelActive)
            .map_err(|_| "转写任务线程不可用".to_string())
    }

    pub fn retry_history_item(
        &self,
        history_id: String,
    ) -> Result<TranscriptionHistoryItem, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.sender
            .send(DispatchMessage::RetryHistory {
                history_id,
                reply: reply_tx,
            })
            .map_err(|_| "转写任务线程不可用".to_string())?;
        reply_rx
            .recv()
            .unwrap_or_else(|_| Err("转写任务线程不可用".to_string()))
    }
}

fn process_retry_source(
    app: &AppHandle,
    store: &SettingsStore,
    source: RetrySource,
    options: processing::ProcessingOptions,
) -> processing::ProcessingOutcome {
    match source {
        RetrySource::Recording(recording) => {
            if let Err(err) = ensure_sensevoice_runtime_ready(app, store) {
                #[cfg(debug_assertions)]
                eprintln!("SenseVoice 运行时自动恢复失败: {err}");
            }
            processing::handle_recording_with_options(store, recording, options)
        }
        RetrySource::TranscribedText {
            text,
            model_group,
            transcription_elapsed_ms,
            recording_duration_ms,
            alignment,
        } => processing::handle_transcribed_text_with_options(
            store,
            &text,
            processing::TranscribedTextContext {
                model_group,
                transcription_elapsed_ms,
                recording_duration_ms,
                alignment,
            },
            options,
        ),
    }
}

fn retry_history_item(
    app: &AppHandle,
    store: &SettingsStore,
    retry_sources: &mut HashMap<String, RetrySource>,
    retry_source_order: &mut VecDeque<String>,
    history_id: &str,
) -> Result<TranscriptionHistoryItem, String> {
    let history = store
        .load_transcription_history()
        .map_err(|err| err.to_string())?;
    let item = history
        .into_iter()
        .find(|item| item.id == history_id)
        .ok_or_else(|| "历史记录不存在，无法重试".to_string())?;
    if item.status != TranscriptionHistoryStatus::Failed {
        return Ok(item);
    }

    let source = retry_sources
        .get(history_id)
        .cloned()
        .or_else(|| retry_source_from_history_item(&item))
        .ok_or_else(|| "原始录音已不可用，无法重试转写".to_string())?;
    let outcome = process_retry_source(
        app,
        store,
        source.clone(),
        processing::ProcessingOptions {
            output_mode: processing::ProcessingOutputMode::PreviewOnly,
            report_status: false,
        },
    );
    let updated = history_item_from_outcome(item.id, item.timestamp_ms, outcome.clone());

    store
        .replace_transcription_history_item(updated.clone())
        .map_err(|err| err.to_string())?;
    if let Err(_err) = app.emit("transcription-history-updated", &updated) {
        #[cfg(debug_assertions)]
        eprintln!("发送历史记录更新事件失败: {_err}");
    }

    if outcome.is_success() {
        retry_sources.remove(&updated.id);
        retry_source_order.retain(|history_id| history_id != &updated.id);
    } else if let Some(next_source) = retry_source_from_previous_outcome(&source, &outcome) {
        remember_retry_source(
            retry_sources,
            retry_source_order,
            updated.id.clone(),
            next_source,
        );
    }

    Ok(updated)
}

fn history_item_from_outcome(
    id: String,
    timestamp_ms: u64,
    outcome: processing::ProcessingOutcome,
) -> TranscriptionHistoryItem {
    TranscriptionHistoryItem {
        id,
        timestamp_ms,
        status: if outcome.is_success() {
            TranscriptionHistoryStatus::Success
        } else {
            TranscriptionHistoryStatus::Failed
        },
        transcription_text: outcome.transcription_text,
        final_text: outcome.final_text,
        model_group: outcome.model_group,
        transcription_elapsed_ms: outcome.transcription_elapsed_ms,
        recording_duration_ms: outcome.recording_duration_ms,
        triggered: outcome.triggered,
        triggered_by_keyword: outcome.triggered_by_keyword,
        trigger_matches: outcome.trigger_matches,
        alignment: outcome.alignment,
        failure_stage: outcome.failure_stage,
        error_message: outcome.error_message,
    }
}

fn replace_history_item(app: &AppHandle, store: &SettingsStore, item: TranscriptionHistoryItem) {
    match store.replace_transcription_history_item(item.clone()) {
        Ok(true) => {
            if let Err(_err) = app.emit("transcription-history-updated", &item) {
                #[cfg(debug_assertions)]
                eprintln!("发送历史记录更新事件失败: {_err}");
            }
        }
        Ok(false) => {}
        Err(_err) => {
            #[cfg(debug_assertions)]
            eprintln!("更新历史记录失败: {_err}");
        }
    }
}

fn retry_source_from_recording_outcome(
    recording: &RecordedAudio,
    outcome: &processing::ProcessingOutcome,
) -> Option<RetrySource> {
    match outcome.failure_stage {
        Some(ProcessingFailureStage::Transcription) => {
            Some(RetrySource::Recording(recording.clone()))
        }
        Some(ProcessingFailureStage::Trigger) => retry_source_from_transcribed_outcome(outcome),
        _ => None,
    }
}

fn retry_source_from_previous_outcome(
    previous: &RetrySource,
    outcome: &processing::ProcessingOutcome,
) -> Option<RetrySource> {
    match outcome.failure_stage {
        Some(ProcessingFailureStage::Transcription) => match previous {
            RetrySource::Recording(recording) => Some(RetrySource::Recording(recording.clone())),
            RetrySource::TranscribedText { .. } => None,
        },
        Some(ProcessingFailureStage::Trigger) => {
            retry_source_from_transcribed_outcome(outcome).or_else(|| Some(previous.clone()))
        }
        _ => None,
    }
}

fn retry_source_from_transcribed_outcome(
    outcome: &processing::ProcessingOutcome,
) -> Option<RetrySource> {
    if outcome.transcription_text.trim().is_empty() {
        return None;
    }
    Some(RetrySource::TranscribedText {
        text: outcome.transcription_text.clone(),
        model_group: outcome.model_group.clone(),
        transcription_elapsed_ms: outcome.transcription_elapsed_ms,
        recording_duration_ms: outcome.recording_duration_ms,
        alignment: outcome.alignment.clone(),
    })
}

fn retry_source_from_history_item(item: &TranscriptionHistoryItem) -> Option<RetrySource> {
    let is_trigger_failure = matches!(item.failure_stage, Some(ProcessingFailureStage::Trigger))
        || item
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("触发词处理失败"));
    if !is_trigger_failure || item.transcription_text.trim().is_empty() {
        return None;
    }
    Some(RetrySource::TranscribedText {
        text: item.transcription_text.clone(),
        model_group: item.model_group.clone(),
        transcription_elapsed_ms: item.transcription_elapsed_ms,
        recording_duration_ms: item.recording_duration_ms,
        alignment: item.alignment.clone(),
    })
}

fn remember_retry_source(
    retry_sources: &mut HashMap<String, RetrySource>,
    retry_source_order: &mut VecDeque<String>,
    history_id: String,
    source: RetrySource,
) {
    if !retry_source_order.iter().any(|value| value == &history_id) {
        retry_source_order.push_back(history_id.clone());
    }
    retry_sources.insert(history_id, source);
    while retry_source_order.len() > RETRY_SOURCE_CACHE_LIMIT {
        if let Some(expired_id) = retry_source_order.pop_front() {
            retry_sources.remove(&expired_id);
        }
    }
}

fn outcome_error_text(outcome: &processing::ProcessingOutcome) -> &str {
    outcome
        .error_message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
        .unwrap_or("")
}

fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn create_history_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("history-{timestamp}")
}

impl Drop for TranscriptionDispatcher {
    fn drop(&mut self) {
        let _ = self.sender.send(DispatchMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// 当前转写提供方为 SenseVoice 时，同步确保本地服务就绪。
/// - 若服务已运行且 HTTP 健康：立即返回。
/// - 否则自动触发 start_service_async（按需新建容器 / unpause / start / 重建镜像），
///   然后轮询等待 `download_state == "ready"` 或 HTTP /health 正常。
/// - 其他转写提供方（云端 API）不做任何处理。
fn ensure_sensevoice_runtime_ready(app: &AppHandle, store: &SettingsStore) -> Result<(), String> {
    let settings = store.load().map_err(|err| err.to_string())?;
    if !settings.provider.is_local() {
        return Ok(());
    }
    ensure_service_ready_blocking(app, store, SENSEVOICE_READY_TIMEOUT)
        .map_err(|err| err.to_string())
}
