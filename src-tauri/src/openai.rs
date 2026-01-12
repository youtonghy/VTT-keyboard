use crate::settings::{OpenAiSettings, Settings};
use reqwest::blocking::{Client, multipart};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OpenAiError {
    #[error("OpenAI 请求失败: {0}")]
    Request(String),
    #[error("OpenAI 响应解析失败: {0}")]
    Parse(String),
    #[error("OpenAI 配置缺失: {0}")]
    Config(String),
    #[error("无法读取音频: {0}")]
    Io(String),
}

#[derive(Serialize)]
struct ResponseRequest<'a> {
    model: &'a str,
    input: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

pub fn transcribe_audio(settings: &Settings, audio_path: &Path) -> Result<String, OpenAiError> {
    ensure_auth(&settings.openai)?;
    let file_bytes = fs::read(audio_path).map_err(|err| OpenAiError::Io(err.to_string()))?;
    let file_name = audio_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("recording.wav");
    let form = build_transcription_form(&settings.openai, file_name, file_bytes)?;
    let client = Client::new();
    let url = format!(
        "{}/audio/transcriptions",
        settings.openai.api_base.trim_end_matches('/')
    );
    let response = client
        .post(url)
        .bearer_auth(settings.openai.api_key.trim())
        .multipart(form)
        .send()
        .map_err(|err| OpenAiError::Request(err.to_string()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(OpenAiError::Request(format!("{status}: {body}")));
    }
    let body = response
        .text()
        .map_err(|err| OpenAiError::Parse(err.to_string()))?;
    if settings.openai.speech_to_text.stream {
        let streamed = parse_streamed_text(&body)?;
        if !streamed.is_empty() {
            return Ok(streamed);
        }
    }
    if settings.openai.speech_to_text.response_format == "text" {
        return Ok(body.trim().to_string());
    }
    let data: TranscriptionResponse =
        serde_json::from_str(&body).map_err(|err| OpenAiError::Parse(err.to_string()))?;
    Ok(data.text)
}

pub fn generate_text(settings: &Settings, input: &str, instructions: &str) -> Result<String, OpenAiError> {
    ensure_auth(&settings.openai)?;
    let request = ResponseRequest {
        model: &settings.openai.text.model,
        input,
        instructions: if instructions.is_empty() { None } else { Some(instructions) },
        max_output_tokens: Some(settings.openai.text.max_output_tokens),
        temperature: Some(settings.openai.text.temperature),
        top_p: Some(settings.openai.text.top_p),
    };
    let client = Client::new();
    let url = format!(
        "{}/responses",
        settings.openai.api_base.trim_end_matches('/')
    );
    let response = client
        .post(url)
        .bearer_auth(settings.openai.api_key.trim())
        .json(&request)
        .send()
        .map_err(|err| OpenAiError::Request(err.to_string()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(OpenAiError::Request(format!("{status}: {body}")));
    }
    let value: Value = response
        .json()
        .map_err(|err| OpenAiError::Parse(err.to_string()))?;
    extract_output_text(&value)
}

fn ensure_auth(settings: &OpenAiSettings) -> Result<(), OpenAiError> {
    if settings.api_key.trim().is_empty() {
        return Err(OpenAiError::Config("API Key 不能为空".to_string()));
    }
    Ok(())
}

fn build_transcription_form(
    settings: &OpenAiSettings,
    filename: &str,
    bytes: Vec<u8>,
) -> Result<multipart::Form, OpenAiError> {
    let mut form = multipart::Form::new()
        .text("model", settings.speech_to_text.model.clone())
        .part("file", multipart::Part::bytes(bytes).file_name(filename.to_string()));

    if !settings.speech_to_text.language.trim().is_empty() {
        form = form.text("language", settings.speech_to_text.language.clone());
    }
    if !settings.speech_to_text.prompt.trim().is_empty() {
        form = form.text("prompt", settings.speech_to_text.prompt.clone());
    }
    if !settings.speech_to_text.response_format.trim().is_empty() {
        form = form.text("response_format", settings.speech_to_text.response_format.clone());
    }
    form = form.text("temperature", settings.speech_to_text.temperature.to_string());

    if settings.speech_to_text.stream {
        form = form.text("stream", "true");
    }
    if !settings.speech_to_text.chunking_strategy.trim().is_empty() {
        form = form.text("chunking_strategy", settings.speech_to_text.chunking_strategy.clone());
    }
    for value in &settings.speech_to_text.include {
        form = form.text("include[]", value.clone());
    }
    for granularity in &settings.speech_to_text.timestamp_granularities {
        form = form.text("timestamp_granularities[]", granularity.clone());
    }
    for name in &settings.speech_to_text.known_speaker_names {
        form = form.text("known_speaker_names[]", name.clone());
    }
    for reference in &settings.speech_to_text.known_speaker_references {
        form = form.text("known_speaker_references[]", reference.clone());
    }

    Ok(form)
}

fn extract_output_text(value: &Value) -> Result<String, OpenAiError> {
    if let Some(text) = value
        .pointer("/output/0/content/0/text")
        .and_then(|val| val.as_str())
    {
        return Ok(text.to_string());
    }
    if let Some(text) = value.get("output_text").and_then(|val| val.as_str()) {
        return Ok(text.to_string());
    }
    Err(OpenAiError::Parse("响应中未找到文本输出".to_string()))
}

fn parse_streamed_text(body: &str) -> Result<String, OpenAiError> {
    let mut output = String::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("data:") {
            continue;
        }
        let payload = trimmed.trim_start_matches("data:").trim();
        if payload == "[DONE]" {
            break;
        }
        let value: Value = serde_json::from_str(payload)
            .map_err(|err| OpenAiError::Parse(err.to_string()))?;
        if let Some(text) = value.get("text").and_then(|val| val.as_str()) {
            output.push_str(text);
        } else if let Some(text) = value
            .pointer("/delta/text")
            .and_then(|val| val.as_str())
        {
            output.push_str(text);
        }
    }
    Ok(output.trim().to_string())
}
