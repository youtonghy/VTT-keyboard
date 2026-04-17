use crate::processing;
use crate::recorder::RecordedAudio;
use crate::sensevoice::ensure_service_ready_blocking;
use crate::settings::{
    SettingsStore, TranscriptionHistoryItem, TranscriptionHistoryStatus, TranscriptionProvider,
};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// 本地 SenseVoice 服务（包括 vLLM）冷启动最长等待时间。
/// vLLM 模型装载可能耗时数分钟，给足 8 分钟的上限以便在系统重启后自动恢复。
const SENSEVOICE_READY_TIMEOUT: Duration = Duration::from_secs(8 * 60);

enum DispatchMessage {
    Process(RecordedAudio),
    Shutdown,
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
            while let Ok(message) = receiver.recv() {
                match message {
                    DispatchMessage::Process(recording) => {
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
                            processing::emit_status("error");
                        }

                        if !outcome.history_enabled {
                            continue;
                        }

                        let item = TranscriptionHistoryItem {
                            id: create_history_id(),
                            timestamp_ms: now_timestamp_ms(),
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
                            error_message: outcome.error_message,
                        };

                        if let Err(_err) = store.append_transcription_history(item.clone()) {
                            #[cfg(debug_assertions)]
                            eprintln!("写入历史记录失败: {_err}");
                            continue;
                        }

                        if let Err(_err) = app.emit("transcription-history-appended", &item) {
                            #[cfg(debug_assertions)]
                            eprintln!("发送历史记录事件失败: {_err}");
                        }
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
    if settings.provider != TranscriptionProvider::Sensevoice {
        return Ok(());
    }
    ensure_service_ready_blocking(app, store, SENSEVOICE_READY_TIMEOUT)
        .map_err(|err| err.to_string())
}
