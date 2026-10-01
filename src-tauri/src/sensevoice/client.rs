use super::{model::normalize_local_model, native_runtime, SenseVoiceError};
use crate::settings::{Settings, TranscriptionAlignment};
use std::path::Path;

pub struct SenseVoiceTranscription {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}

/// 本地模型转写：全部通过 sherpa-onnx 进程内推理完成，不发起网络请求。
pub fn transcribe_audio(
    settings: &Settings,
    audio_path: &Path,
) -> Result<SenseVoiceTranscription, SenseVoiceError> {
    if !settings.sensevoice.enabled {
        return Err(SenseVoiceError::Config(
            "本地模型尚未启用，请先下载并启用".to_string(),
        ));
    }
    let local_model = normalize_local_model(&settings.sensevoice.local_model);
    if settings
        .sensevoice
        .download_state
        .trim()
        .eq_ignore_ascii_case("downloading")
    {
        return Err(SenseVoiceError::Request(
            "本地模型正在下载中，请稍后重试".to_string(),
        ));
    }
    // 就绪状态以磁盘上的模型文件为准，兼容旧配置迁移后 installed 标记残留的情况
    if !native_runtime::model_files_ready(local_model) {
        return Err(SenseVoiceError::Config(
            "本地模型尚未安装，请先下载模型".to_string(),
        ));
    }

    let result =
        native_runtime::transcribe_wav(local_model, &settings.sensevoice.language, audio_path)?;
    Ok(SenseVoiceTranscription {
        text: result.text,
        alignment: result.alignment,
    })
}
