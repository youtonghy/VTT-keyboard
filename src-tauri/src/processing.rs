use crate::audio_processing::{self, AudioProcessingError};
use crate::openai::{self, OpenAiError};
use crate::paste::{self, PasteError};
use crate::recorder::RecordedAudio;
use crate::sensevoice::{self, SenseVoiceError};
use crate::settings::{SettingsError, SettingsStore, TranscriptionProvider};
use crate::status_native::{self, StatusType};
use crate::triggers;
use crate::volcengine::{self, VolcengineError};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;
use thiserror::Error;

/// Counter to track status show operations, used to prevent race conditions
/// when hiding the status window after a delay.
static STATUS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn dev_log(message: &str) {
    #[cfg(debug_assertions)]
    {
        eprintln!("[DEV] {}", message);
    }
}

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("设置读取失败: {0}")]
    Settings(#[from] SettingsError),
    #[error("录音分段失败: {0}")]
    Audio(#[from] AudioProcessingError),
    #[error("OpenAI 转写失败: {0}")]
    OpenAi(#[from] OpenAiError),
    #[error("火山引擎转写失败: {0}")]
    Volcengine(#[from] VolcengineError),
    #[error("SenseVoice 转写失败: {0}")]
    SenseVoice(#[from] SenseVoiceError),
    #[error("触发词处理失败: {0}")]
    Trigger(String),
    #[error("写入剪贴板失败: {0}")]
    Paste(#[from] PasteError),
}

pub fn handle_recording(
    store: &SettingsStore,
    recording: RecordedAudio,
) -> Result<(), ProcessingError> {
    let settings = store.load()?;
    if recording.samples.is_empty() {
        dev_log("录音为空，跳过转写");
        emit_status("completed");
        return Ok(());
    }
    let segment_seconds = settings.recording.segment_seconds.max(1);
    dev_log(&format!(
        "开始转写，采样 {}，分段秒数 {}",
        recording.samples.len(),
        segment_seconds
    ));
    let paths = audio_processing::write_segments(&recording, segment_seconds)?;
    dev_log(&format!("生成 {} 段录音", paths.len()));

    let mut transcripts = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        dev_log(&format!("开始请求转写段落 {}", index + 1));
        let text = match settings.provider {
            TranscriptionProvider::Openai => openai::transcribe_audio(&settings, path)?,
            TranscriptionProvider::Volcengine => volcengine::transcribe_audio(&settings, path)?,
            TranscriptionProvider::Sensevoice => sensevoice::client::transcribe_audio(&settings, path)?,
        };
        dev_log(&format!("转写结果 {}: {}", index + 1, text));
        transcripts.push(text);
    }

    cleanup_files(&paths);

    let combined = transcripts.join(" ");
    dev_log(&format!("合并转写结果: {}", combined));
    let logger = |message: &str| dev_log(message);
    let result = triggers::apply_triggers(&settings, &combined, &logger)
        .map_err(|err| ProcessingError::Trigger(err.to_string()))?;
    dev_log(&format!(
        "触发词处理: {}",
        if result.triggered { "已触发" } else { "未触发" }
    ));
    dev_log(&format!("触发词输出: {}", result.output));

    if result.triggered {
        dev_log("复制原文到剪贴板");
        paste::write_text(&combined)?;
    }
    dev_log("写入并粘贴处理后的文本");
    paste::write_and_paste(&result.output)?;
    emit_status("completed");
    Ok(())
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
