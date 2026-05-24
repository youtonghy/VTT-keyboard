use crate::privacy;
use crate::settings::{OpenAiSettings, Settings, TextSettings};
use reqwest::blocking::{multipart, Client};
use reqwest::StatusCode;
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

#[derive(Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Clone, Copy)]
enum TextEndpoint {
    Responses,
    ChatCompletions,
}

impl TextEndpoint {
    fn path(self) -> &'static str {
        match self {
            TextEndpoint::Responses => "responses",
            TextEndpoint::ChatCompletions => "chat/completions",
        }
    }
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

pub fn transcribe_audio(settings: &Settings, audio_path: &Path) -> Result<String, OpenAiError> {
    ensure_auth(&settings.openai)?;
    privacy::ensure_url_allowed(&settings.privacy, &settings.openai.api_base, "OpenAI 转写")
        .map_err(|err| OpenAiError::Config(err.to_string()))?;
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

pub fn generate_text(
    settings: &TextSettings,
    input: &str,
    instructions: &str,
) -> Result<String, OpenAiError> {
    ensure_text_auth(settings)?;
    let client = Client::new();

    if prefers_chat_completions(&settings.api_base) {
        return generate_text_with_chat_completions(&client, settings, input, instructions);
    }

    let request = build_response_request(settings, input, instructions);
    let url = text_endpoint_url(&settings.api_base, TextEndpoint::Responses);
    let response = post_json(&client, &url, settings.api_key.trim(), &request)?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if should_retry_with_chat_completions(status, &body) {
            return generate_text_with_chat_completions(&client, settings, input, instructions)
                .map_err(|err| {
                    OpenAiError::Request(format!(
                        "{status}: {body}; Chat Completions fallback failed: {err}"
                    ))
                });
        }
        return Err(OpenAiError::Request(format!("{status}: {body}")));
    }
    let value: Value = response
        .json()
        .map_err(|err| OpenAiError::Parse(err.to_string()))?;
    extract_output_text(&value)
}

fn generate_text_with_chat_completions(
    client: &Client,
    settings: &TextSettings,
    input: &str,
    instructions: &str,
) -> Result<String, OpenAiError> {
    let request = build_chat_completion_request(settings, input, instructions);
    let url = text_endpoint_url(&settings.api_base, TextEndpoint::ChatCompletions);
    let response = post_json(client, &url, settings.api_key.trim(), &request)?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(OpenAiError::Request(format!("{status}: {body}")));
    }
    let value: Value = response
        .json()
        .map_err(|err| OpenAiError::Parse(err.to_string()))?;
    extract_chat_completion_text(&value)
}

pub fn generate_text_for_settings(
    settings: &Settings,
    input: &str,
    instructions: &str,
) -> Result<String, OpenAiError> {
    privacy::ensure_url_allowed(
        &settings.privacy,
        &settings.text_processing.openai.api_base,
        "触发词处理",
    )
    .map_err(|err| OpenAiError::Config(err.to_string()))?;
    generate_text(&settings.text_processing.openai, input, instructions)
}

fn ensure_auth(settings: &OpenAiSettings) -> Result<(), OpenAiError> {
    if settings.api_key.trim().is_empty() {
        return Err(OpenAiError::Config("API Key 不能为空".to_string()));
    }
    Ok(())
}

fn ensure_text_auth(settings: &TextSettings) -> Result<(), OpenAiError> {
    if settings.api_key.trim().is_empty() {
        return Err(OpenAiError::Config("文本处理 API Key 不能为空".to_string()));
    }
    Ok(())
}

fn build_response_request<'a>(
    settings: &'a TextSettings,
    input: &'a str,
    instructions: &'a str,
) -> ResponseRequest<'a> {
    let instructions = instructions.trim();
    ResponseRequest {
        model: &settings.model,
        input,
        instructions: if instructions.is_empty() {
            None
        } else {
            Some(instructions)
        },
        max_output_tokens: Some(settings.max_output_tokens),
        temperature: Some(settings.temperature),
        top_p: Some(settings.top_p),
    }
}

fn build_chat_completion_request<'a>(
    settings: &'a TextSettings,
    input: &'a str,
    instructions: &'a str,
) -> ChatCompletionRequest<'a> {
    let instructions = instructions.trim();
    let mut messages = Vec::new();
    if !instructions.is_empty() {
        messages.push(ChatMessage {
            role: "system",
            content: instructions,
        });
    }
    messages.push(ChatMessage {
        role: "user",
        content: input,
    });

    ChatCompletionRequest {
        model: &settings.model,
        messages,
        max_tokens: Some(settings.max_output_tokens),
        temperature: Some(settings.temperature),
        top_p: Some(settings.top_p),
    }
}

fn post_json<T: Serialize + ?Sized>(
    client: &Client,
    url: &str,
    api_key: &str,
    request: &T,
) -> Result<reqwest::blocking::Response, OpenAiError> {
    client
        .post(url)
        .bearer_auth(api_key)
        .json(request)
        .send()
        .map_err(|err| OpenAiError::Request(err.to_string()))
}

fn prefers_chat_completions(api_base: &str) -> bool {
    api_base
        .trim()
        .trim_end_matches('/')
        .ends_with("/chat/completions")
}

fn should_retry_with_chat_completions(status: StatusCode, body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    let expects_messages = body.contains("missing field") && body.contains("messages");
    let responses_unsupported =
        matches!(
            status,
            StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
        ) && (body.is_empty() || body.contains("responses") || body.contains("not found"));

    (status == StatusCode::BAD_REQUEST && expects_messages) || responses_unsupported
}

fn text_endpoint_url(api_base: &str, endpoint: TextEndpoint) -> String {
    let base = api_base.trim().trim_end_matches('/');
    let root = base
        .strip_suffix("/chat/completions")
        .or_else(|| base.strip_suffix("/responses"))
        .unwrap_or(base)
        .trim_end_matches('/');

    format!("{}/{}", root, endpoint.path())
}

fn build_transcription_form(
    settings: &OpenAiSettings,
    filename: &str,
    bytes: Vec<u8>,
) -> Result<multipart::Form, OpenAiError> {
    let mut form = multipart::Form::new()
        .text("model", settings.speech_to_text.model.clone())
        .part(
            "file",
            multipart::Part::bytes(bytes).file_name(filename.to_string()),
        );

    if !settings.speech_to_text.language.trim().is_empty() {
        form = form.text("language", settings.speech_to_text.language.clone());
    }
    if !settings.speech_to_text.prompt.trim().is_empty() {
        form = form.text("prompt", settings.speech_to_text.prompt.clone());
    }
    if !settings.speech_to_text.response_format.trim().is_empty() {
        form = form.text(
            "response_format",
            settings.speech_to_text.response_format.clone(),
        );
    }
    form = form.text(
        "temperature",
        settings.speech_to_text.temperature.to_string(),
    );

    if settings.speech_to_text.stream {
        form = form.text("stream", "true");
    }
    if !settings.speech_to_text.chunking_strategy.trim().is_empty() {
        form = form.text(
            "chunking_strategy",
            settings.speech_to_text.chunking_strategy.clone(),
        );
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

fn extract_chat_completion_text(value: &Value) -> Result<String, OpenAiError> {
    let content = value.pointer("/choices/0/message/content");
    if let Some(text) = content.and_then(|val| val.as_str()) {
        return Ok(text.to_string());
    }
    if let Some(parts) = content.and_then(|val| val.as_array()) {
        let output = parts
            .iter()
            .filter_map(|part| part.get("text").and_then(|val| val.as_str()))
            .collect::<String>();
        if !output.is_empty() {
            return Ok(output);
        }
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
        let value: Value =
            serde_json::from_str(payload).map_err(|err| OpenAiError::Parse(err.to_string()))?;
        if let Some(text) = value.get("text").and_then(|val| val.as_str()) {
            output.push_str(text);
        } else if let Some(text) = value.pointer("/delta/text").and_then(|val| val.as_str()) {
            output.push_str(text);
        }
    }
    Ok(output.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text_settings() -> TextSettings {
        TextSettings {
            api_base: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.5,
            max_output_tokens: 800,
            top_p: 1.0,
            instructions: String::new(),
        }
    }

    #[test]
    fn response_request_uses_input_and_instructions() {
        let settings = text_settings();
        let request = build_response_request(&settings, "hello", "Be concise.");
        let value = serde_json::to_value(request).expect("serialize response request");

        assert_eq!(
            value,
            json!({
                "model": "gpt-4o-mini",
                "input": "hello",
                "instructions": "Be concise.",
                "max_output_tokens": 800,
                "temperature": 0.5,
                "top_p": 1.0
            })
        );
    }

    #[test]
    fn response_request_omits_blank_instructions() {
        let settings = text_settings();
        let request = build_response_request(&settings, "hello", "  ");
        let value = serde_json::to_value(request).expect("serialize response request");

        assert!(value.get("instructions").is_none());
    }

    #[test]
    fn chat_completion_request_uses_messages() {
        let settings = text_settings();
        let request = build_chat_completion_request(&settings, "hello", "Be concise.");
        let value = serde_json::to_value(request).expect("serialize chat request");

        assert_eq!(
            value,
            json!({
                "model": "gpt-4o-mini",
                "messages": [
                    { "role": "system", "content": "Be concise." },
                    { "role": "user", "content": "hello" }
                ],
                "max_tokens": 800,
                "temperature": 0.5,
                "top_p": 1.0
            })
        );
    }

    #[test]
    fn text_endpoint_url_normalizes_explicit_endpoint_paths() {
        assert_eq!(
            text_endpoint_url(
                "https://api.example.com/v1/chat/completions",
                TextEndpoint::Responses
            ),
            "https://api.example.com/v1/responses"
        );
        assert_eq!(
            text_endpoint_url(
                "https://api.example.com/v1/responses",
                TextEndpoint::ChatCompletions
            ),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn bad_request_missing_messages_retries_with_chat_completions() {
        let body = r#"{"error":{"message":"missing field `messages`"}}"#;
        assert!(should_retry_with_chat_completions(
            StatusCode::BAD_REQUEST,
            body
        ));
    }

    #[test]
    fn chat_completion_text_extracts_string_content() {
        let value = json!({
            "choices": [
                { "message": { "content": "polished text" } }
            ]
        });

        assert_eq!(
            extract_chat_completion_text(&value).expect("chat text"),
            "polished text"
        );
    }

    #[test]
    fn chat_completion_text_extracts_text_parts() {
        let value = json!({
            "choices": [
                {
                    "message": {
                        "content": [
                            { "type": "text", "text": "polished " },
                            { "type": "text", "text": "text" }
                        ]
                    }
                }
            ]
        });

        assert_eq!(
            extract_chat_completion_text(&value).expect("chat text"),
            "polished text"
        );
    }
}
