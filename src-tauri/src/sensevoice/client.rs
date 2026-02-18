use super::SenseVoiceError;
use crate::settings::Settings;
use reqwest::blocking::{multipart, Client};
use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

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

    let client = Client::new();
    for attempt in 0..2 {
        let form = multipart::Form::new()
            .part(
                "file",
                multipart::Part::bytes(file_bytes.clone()).file_name(file_name.to_string()),
            )
            .text("language", "auto".to_string());

        let response = client
            .post(format!("{service_url}/api/v1/asr"))
            .multipart(form)
            .send()
            .map_err(|err| SenseVoiceError::Request(err.to_string()))?;

        if response.status().is_success() {
            let data: SenseVoiceResponse = response
                .json()
                .map_err(|err| SenseVoiceError::Parse(err.to_string()))?;
            return Ok(data.text);
        }

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if attempt == 0 && status.as_u16() == 503 && is_warming_up_error(&body) {
            thread::sleep(Duration::from_secs(2));
            continue;
        }
        return Err(SenseVoiceError::Request(format!("{status}: {body}")));
    }

    Err(SenseVoiceError::Request(
        "SenseVoice 服务暂不可用，请稍后重试".to_string(),
    ))
}

fn is_warming_up_error(body: &str) -> bool {
    let lowered = body.to_lowercase();
    lowered.contains("warming")
        || lowered.contains("loading")
        || lowered.contains("retry")
        || lowered.contains("预热")
}
