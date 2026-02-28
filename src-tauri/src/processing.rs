use crate::audio_processing::{self, AudioProcessingError};
use crate::paste::{self, PasteError};
use crate::recorder::RecordedAudio;
use crate::sensevoice;
use crate::settings::{SettingsStore, TranscriptionProvider, TriggerMatch};
use crate::status_native::{self, StatusType};
use crate::triggers;
use crate::volcengine;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

/// Counter to track status show operations, used to prevent race conditions
/// when hiding the status window after a delay.
static STATUS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn dev_log(message: &str) {
    #[cfg(debug_assertions)]
    {
        eprintln!("[DEV] {}", message);
    }
}

#[derive(Clone)]
pub struct ProcessingOutcome {
    pub history_enabled: bool,
    pub transcription_text: String,
    pub final_text: String,
    pub triggered: bool,
    pub triggered_by_keyword: bool,
    pub trigger_matches: Vec<TriggerMatch>,
    pub error_message: Option<String>,
}

impl ProcessingOutcome {
    fn success(
        history_enabled: bool,
        transcription_text: String,
        final_text: String,
        triggered: bool,
        triggered_by_keyword: bool,
        trigger_matches: Vec<TriggerMatch>,
    ) -> Self {
        Self {
            history_enabled,
            transcription_text,
            final_text,
            triggered,
            triggered_by_keyword,
            trigger_matches,
            error_message: None,
        }
    }

    fn failed(
        history_enabled: bool,
        transcription_text: String,
        final_text: String,
        triggered: bool,
        triggered_by_keyword: bool,
        trigger_matches: Vec<TriggerMatch>,
        error_message: String,
    ) -> Self {
        Self {
            history_enabled,
            transcription_text,
            final_text,
            triggered,
            triggered_by_keyword,
            trigger_matches,
            error_message: Some(error_message),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error_message.is_none()
    }
}

pub fn handle_recording(store: &SettingsStore, recording: RecordedAudio) -> ProcessingOutcome {
    let settings = match store.load() {
        Ok(value) => value,
        Err(err) => {
            return ProcessingOutcome::failed(
                false,
                String::new(),
                String::new(),
                false,
                false,
                Vec::new(),
                format!("设置读取失败: {err}"),
            )
        }
    };
    let history_enabled = settings.history.enabled;

    if recording.samples.is_empty() {
        dev_log("录音为空，跳过转写");
        emit_status("completed");
        return ProcessingOutcome::success(
            history_enabled,
            String::new(),
            String::new(),
            false,
            false,
            Vec::new(),
        );
    }
    let segment_seconds = settings.recording.segment_seconds.max(1);
    dev_log(&format!(
        "开始转写，采样 {}，分段秒数 {}",
        recording.samples.len(),
        segment_seconds
    ));
    let paths = match audio_processing::write_segments(&recording, segment_seconds) {
        Ok(value) => value,
        Err(err) => {
            return processing_audio_error(history_enabled, err);
        }
    };
    dev_log(&format!("生成 {} 段录音", paths.len()));

    let mut transcripts = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        dev_log(&format!("开始请求转写段落 {}", index + 1));
        let text = match settings.provider {
            TranscriptionProvider::Openai => crate::openai::transcribe_audio(&settings, path)
                .map_err(|err| format!("OpenAI 转写失败: {err}")),
            TranscriptionProvider::Volcengine => volcengine::transcribe_audio(&settings, path)
                .map_err(|err| format!("火山引擎转写失败: {err}")),
            TranscriptionProvider::Sensevoice => {
                sensevoice::client::transcribe_audio(&settings, path)
                    .map_err(|err| format!("SenseVoice 转写失败: {err}"))
            }
        };
        let text = match text {
            Ok(value) => value,
            Err(message) => {
                cleanup_files(&paths);
                let partial = transcripts.join(" ");
                return ProcessingOutcome::failed(
                    history_enabled,
                    partial.clone(),
                    partial,
                    false,
                    false,
                    Vec::new(),
                    message,
                );
            }
        };
        dev_log(&format!("转写结果 {}: {}", index + 1, text));
        transcripts.push(text);
    }

    cleanup_files(&paths);

    let combined = transcripts.join(" ");
    dev_log(&format!("合并转写结果: {}", combined));
    let logger = |message: &str| dev_log(message);
    let result = match triggers::apply_triggers(&settings, &combined, &logger) {
        Ok(value) => value,
        Err(err) => {
            return ProcessingOutcome::failed(
                history_enabled,
                combined.clone(),
                combined,
                false,
                false,
                Vec::new(),
                format!("触发词处理失败: {err}"),
            );
        }
    };
    dev_log(&format!(
        "触发词处理: {}",
        if result.triggered { "已触发" } else { "未触发" }
    ));
    dev_log(&format!("触发词输出: {}", result.output));

    if result.triggered {
        dev_log("复制原文到剪贴板");
        if let Err(err) = paste::write_text(&combined) {
            return processing_paste_error(
                history_enabled,
                &combined,
                &result.output,
                result.triggered,
                result.triggered_by_keyword,
                result.trigger_matches,
                err,
            );
        }
    }
    dev_log("写入并粘贴处理后的文本");
    if let Err(err) = paste::write_and_paste(&result.output) {
        return processing_paste_error(
            history_enabled,
            &combined,
            &result.output,
            result.triggered,
            result.triggered_by_keyword,
            result.trigger_matches,
            err,
        );
    }
    emit_status("completed");
    ProcessingOutcome::success(
        history_enabled,
        combined,
        result.output,
        result.triggered,
        result.triggered_by_keyword,
        result.trigger_matches,
    )
}

/// Show status overlay with native window.
/// For "completed" and "error" status, auto-hide after 2 seconds.
pub fn emit_status(status: &str) {
    let (status_type, text) = match status {
        "recording" => (StatusType::Recording, "正在录音"),
        "transcribing" => (StatusType::Transcribing, "正在转写"),
        "completed" => (StatusType::Completed, "已完成"),
        "error" => (StatusType::Error, "已中断"),
        _ => return,
    };

    // Increment counter to invalidate any pending hide operations
    let current_count = STATUS_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

    status_native::show(status_type, text);

    // Auto-hide after 2 seconds for completed/error states
    // Only hide if no new status was shown during the delay
    if status_type == StatusType::Completed || status_type == StatusType::Error {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            // Only hide if the counter hasn't changed (no new status was shown)
            if STATUS_COUNTER.load(Ordering::SeqCst) == current_count {
                status_native::hide();
            }
        });
    }
}

fn cleanup_files(paths: &[std::path::PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn processing_audio_error(history_enabled: bool, err: AudioProcessingError) -> ProcessingOutcome {
    ProcessingOutcome::failed(
        history_enabled,
        String::new(),
        String::new(),
        false,
        false,
        Vec::new(),
        format!("录音分段失败: {err}"),
    )
}

fn processing_paste_error(
    history_enabled: bool,
    transcription_text: &str,
    final_text: &str,
    triggered: bool,
    triggered_by_keyword: bool,
    trigger_matches: Vec<TriggerMatch>,
    err: PasteError,
) -> ProcessingOutcome {
    ProcessingOutcome::failed(
        history_enabled,
        transcription_text.to_string(),
        final_text.to_string(),
        triggered,
        triggered_by_keyword,
        trigger_matches,
        format!("写入剪贴板失败: {err}"),
    )
}
