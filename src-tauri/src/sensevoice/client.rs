use super::SenseVoiceError;
use crate::settings::Settings;
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct SenseVoiceResponse {
    text: String,
}

pub fn transcribe_audio(settings: &Settings, audio_path: &Path) -> Result<String, SenseVoiceError> {
    if !settings.sensevoice.enabled {
        return Err(SenseVoiceError::Config(
            "SenseVoice 尚未启用，请先下载并启用".to_string(),
        ));
    }
    if !settings.sensevoice.installed {
        return Err(SenseVoiceError::Config(
            "SenseVoice 尚未安装，请先下载模型".to_string(),
        ));
    }

    let service_url = settings.sensevoice.service_url.trim().trim_end_matches('/');
    if service_url.is_empty() {
        return Err(SenseVoiceError::Config(
            "SenseVoice 服务地址不能为空".to_string(),
        ));
    }

    let file_bytes = fs::read(audio_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    let file_name = audio_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("recording.wav");

    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(file_bytes).file_name(file_name.to_string()),
        )
        .text("language", "auto".to_string());

    let client = Client::new();
    let response = client
        .post(format!("{service_url}/api/v1/asr"))
        .multipart(form)
        .send()
        .map_err(|err| SenseVoiceError::Request(err.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(SenseVoiceError::Request(format!("{status}: {body}")));
    }

    let data: SenseVoiceResponse = response
        .json()
        .map_err(|err| SenseVoiceError::Parse(err.to_string()))?;
    Ok(data.text)
}
