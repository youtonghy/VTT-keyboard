use super::{
    model::{normalize_local_model, resolve_vllm_model_id, spec_for_local_model, LocalRuntimeKind},
    native_runtime, SenseVoiceError,
};
use crate::settings::{Settings, TranscriptionAlignment};
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

pub struct SenseVoiceTranscription {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}

pub fn transcribe_audio(
    settings: &Settings,
    audio_path: &Path,
) -> Result<SenseVoiceTranscription, SenseVoiceError> {
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
    if settings
        .sensevoice
        .download_state
        .trim()
        .eq_ignore_ascii_case("running")
    {
        return Err(SenseVoiceError::Request(
            "SenseVoice 模型正在预热中，请稍后重试".to_string(),
        ));
    }

    let local_model = normalize_local_model(&settings.sensevoice.local_model);
    let local_model_spec = spec_for_local_model(local_model);
    if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
        let result = native_runtime::transcribe_wav(
            local_model,
            &settings.sensevoice.language,
            audio_path,
        )?;
        return Ok(SenseVoiceTranscription {
            text: result.text,
            alignment: result.alignment,
        });
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
        let mut form = multipart::Form::new()
            .part(
                "file",
                multipart::Part::bytes(file_bytes.clone()).file_name(file_name.to_string()),
            );
        let endpoint = if local_model == "sensevoice" {
            form = form.text("language", "auto".to_string());
            "/api/v1/asr"
        } else {
            let model_id = resolve_vllm_model_id(local_model, &settings.sensevoice.model_id);
            form = form
                .text("model", model_id)
                .text("response_format", "json".to_string());
            "/v1/audio/transcriptions"
        };

        let response = client
            .post(format!("{service_url}{endpoint}"))
            .multipart(form)
            .send()
            .map_err(|err| SenseVoiceError::Request(err.to_string()))?;

        if response.status().is_success() {
            let data: SenseVoiceResponse = response
                .json()
                .map_err(|err| SenseVoiceError::Parse(err.to_string()))?;
            return Ok(SenseVoiceTranscription {
                text: data.text,
                alignment: None,
            });
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
