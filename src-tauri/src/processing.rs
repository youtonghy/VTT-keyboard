use crate::audio_processing::{self, AudioProcessingError};
use crate::openai::{self, OpenAiError};
use crate::paste::{self, PasteError};
use crate::recorder::RecordedAudio;
use crate::settings::{SettingsError, SettingsStore};
use crate::triggers;
use std::fs;
use thiserror::Error;
use tauri::{AppHandle, Emitter};

fn emit_dev_log(app: &AppHandle, message: &str) {
    #[cfg(debug_assertions)]
    {
        let _ = app.emit_to("main", "dev-log", message.to_string());
    }
}

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("设置读取失败: {0}")]
    Settings(#[from] SettingsError),
    #[error("录音分段失败: {0}")]
    Audio(#[from] AudioProcessingError),
    #[error("转写失败: {0}")]
    OpenAi(#[from] OpenAiError),
    #[error("触发词处理失败: {0}")]
    Trigger(String),
    #[error("写入剪贴板失败: {0}")]
    Paste(#[from] PasteError),
}

pub fn handle_recording(
    app: &AppHandle,
    store: &SettingsStore,
    recording: RecordedAudio,
) -> Result<(), ProcessingError> {
    let settings = store.load()?;
    if recording.samples.is_empty() {
        emit_dev_log(app, "录音为空，跳过转写");
        emit_status(app, "completed");
        return Ok(());
    }
    let segment_seconds = settings.recording.segment_seconds.max(1);
    emit_dev_log(
        app,
        &format!(
            "开始转写，采样 {}，分段秒数 {}",
            recording.samples.len(),
            segment_seconds
        ),
    );
    let paths = audio_processing::write_segments(app, &recording, segment_seconds)?;
    emit_dev_log(app, &format!("生成 {} 段录音", paths.len()));

    let mut transcripts = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        emit_dev_log(app, &format!("开始请求转写段落 {}", index + 1));
        let text = openai::transcribe_audio(&settings, path)?;
        emit_dev_log(app, &format!("转写结果 {}: {}", index + 1, text));
        transcripts.push(text);
    }

    cleanup_files(&paths);

    let combined = transcripts.join(" ");
    emit_dev_log(app, &format!("合并转写结果: {}", combined));
    let logger = |message: &str| emit_dev_log(app, message);
    let result = triggers::apply_triggers(&settings, &combined, &logger)
        .map_err(|err| ProcessingError::Trigger(err.to_string()))?;
    emit_dev_log(
        app,
        &format!(
            "触发词处理: {}",
            if result.triggered { "已触发" } else { "未触发" }
        ),
    );
    emit_dev_log(app, &format!("触发词输出: {}", result.output));

    if result.triggered {
        emit_dev_log(app, "复制原文到剪贴板");
        paste::write_text(app, &combined)?;
    }
    emit_dev_log(app, "写入并粘贴处理后的文本");
    paste::write_and_paste(app, &result.output)?;
    emit_status(app, "completed");
    Ok(())
}

pub fn emit_status(app: &AppHandle, status: &str) {
    let _ = app.emit_to("status", "status-update", status);
}

fn cleanup_files(paths: &[std::path::PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}
