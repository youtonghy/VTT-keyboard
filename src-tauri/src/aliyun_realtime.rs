use crate::settings::Settings;
use hound::{SampleFormat, WavReader};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use thiserror::Error;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::{connect, Message, WebSocket};
use url::Url;

const WS_ENDPOINT_BEIJING: &str = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";
const WS_ENDPOINT_SINGAPORE: &str = "wss://dashscope-intl.aliyuncs.com/api-ws/v1/inference";
const ALIYUN_REGION_BEIJING: &str = "beijing";
const ALIYUN_REGION_SINGAPORE: &str = "singapore";
const MODEL_FUN_ASR_REALTIME_V2: &str = "fun-asr-realtime-v2";
const MODEL_PARAFORMER_REALTIME_V2: &str = "paraformer-realtime-v2";
const CHUNK_SIZE_BYTES: usize = 3200;

#[derive(Debug, Error)]
pub enum AliyunRealtimeError {
    #[error("阿里云配置缺失: {0}")]
    Config(String),
    #[error("阿里云请求失败: {0}")]
    Request(String),
    #[error("阿里云响应解析失败: {0}")]
    Parse(String),
    #[error("无法读取音频: {0}")]
    Io(String),
    #[error("WebSocket 错误: {0}")]
    WebSocket(String),
}

#[derive(Clone, Copy)]
enum ProviderKind {
    FunAsr,
    Paraformer,
}

struct Segment {
    begin_time: Option<u64>,
    order: u64,
    text: String,
}

#[derive(Debug)]
enum ServerEventAction {
    Continue,
    TaskStarted,
    TaskFinished,
}

pub fn transcribe_asr(settings: &Settings, audio_path: &Path) -> Result<String, AliyunRealtimeError> {
    transcribe_realtime(settings, audio_path, ProviderKind::FunAsr)
}

pub fn transcribe_paraformer(
    settings: &Settings,
    audio_path: &Path,
) -> Result<String, AliyunRealtimeError> {
    transcribe_realtime(settings, audio_path, ProviderKind::Paraformer)
}

fn transcribe_realtime(
    settings: &Settings,
    audio_path: &Path,
    provider: ProviderKind,
) -> Result<String, AliyunRealtimeError> {
    let region = normalize_region(&settings.aliyun.region);
    if matches!(provider, ProviderKind::Paraformer) && region == ALIYUN_REGION_SINGAPORE {
        return Err(AliyunRealtimeError::Config(
            "Paraformer 仅支持北京地域".to_string(),
        ));
    }
    let api_key = resolve_api_key(settings, region)?;
    let endpoint = resolve_endpoint(region)?;

    let mut request = endpoint
        .as_str()
        .into_client_request()
        .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("bearer {api_key}"))
            .map_err(|err| AliyunRealtimeError::Config(err.to_string()))?,
    );
    request.headers_mut().insert(
        "X-DashScope-DataInspection",
        HeaderValue::from_static("disable"),
    );
    let (mut socket, _) = connect(request).map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;

    let task_id = uuid_simple();
    let start_message = build_run_task_message(settings, provider, &task_id);
    socket
        .send(Message::Text(start_message))
        .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
    wait_for_task_started(&mut socket)?;

    let payload = read_wav_as_pcm16k_mono(audio_path)?;
    for chunk in payload.chunks(CHUNK_SIZE_BYTES) {
        socket
            .send(Message::Binary(chunk.to_vec()))
            .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
    }

    let finish_message = json!({
        "header": {
            "action": "finish-task",
            "task_id": task_id,
            "streaming": "duplex"
        },
        "payload": {
            "input": {}
        }
    })
    .to_string();
    socket
        .send(Message::Text(finish_message))
        .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;

    let text = collect_transcription_result(&mut socket)?;
    let _ = socket.close(None);
    Ok(text)
}

fn resolve_api_key(settings: &Settings, region: &str) -> Result<String, AliyunRealtimeError> {
    let key = if region == ALIYUN_REGION_SINGAPORE {
        settings.aliyun.api_keys.singapore.trim()
    } else {
        settings.aliyun.api_keys.beijing.trim()
    };
    if key.is_empty() {
        return Err(AliyunRealtimeError::Config(
            "当前地域的阿里云 API Key 不能为空".to_string(),
        ));
    }
    Ok(key.to_string())
}

fn resolve_endpoint(region: &str) -> Result<Url, AliyunRealtimeError> {
    let endpoint = if region == ALIYUN_REGION_SINGAPORE {
        WS_ENDPOINT_SINGAPORE
    } else {
        WS_ENDPOINT_BEIJING
    };
    Url::parse(endpoint).map_err(|err| AliyunRealtimeError::Config(err.to_string()))
}

fn normalize_region(region: &str) -> &str {
    if region.eq_ignore_ascii_case(ALIYUN_REGION_SINGAPORE) {
        ALIYUN_REGION_SINGAPORE
    } else {
        ALIYUN_REGION_BEIJING
    }
}

fn build_run_task_message(settings: &Settings, provider: ProviderKind, task_id: &str) -> String {
    let mut parameters = json!({
        "format": "pcm",
        "sample_rate": 16000,
        "semantic_punctuation_enabled": true
    });

    match provider {
        ProviderKind::FunAsr => {
            if let Some(vocabulary_id) = non_empty(&settings.aliyun.asr.vocabulary_id) {
                parameters["vocabulary_id"] = Value::String(vocabulary_id.to_string());
            }
        }
        ProviderKind::Paraformer => {
            if let Some(vocabulary_id) = non_empty(&settings.aliyun.paraformer.vocabulary_id) {
                parameters["vocabulary_id"] = Value::String(vocabulary_id.to_string());
            }
            if !settings.aliyun.paraformer.language_hints.is_empty() {
                parameters["language_hints"] =
                    serde_json::to_value(&settings.aliyun.paraformer.language_hints)
                        .unwrap_or(Value::Array(Vec::new()));
            }
        }
    }

    json!({
        "header": {
            "action": "run-task",
            "task_id": task_id,
            "streaming": "duplex"
        },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": match provider {
                ProviderKind::FunAsr => MODEL_FUN_ASR_REALTIME_V2,
                ProviderKind::Paraformer => MODEL_PARAFORMER_REALTIME_V2
            },
            "parameters": parameters,
            "input": {}
        }
    })
    .to_string()
}

fn wait_for_task_started(
    socket: &mut WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) -> Result<(), AliyunRealtimeError> {
    let mut segments = Vec::new();
    let mut sequence = 0u64;
    loop {
        let message = socket
            .read()
            .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
        match message {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|err| AliyunRealtimeError::Parse(err.to_string()))?;
                match handle_server_event(&value, &mut segments, &mut sequence)? {
                    ServerEventAction::TaskStarted => return Ok(()),
                    ServerEventAction::TaskFinished => {
                        return Err(AliyunRealtimeError::Request(
                            "服务端在任务启动前结束了任务".to_string(),
                        ))
                    }
                    ServerEventAction::Continue => {}
                }
            }
            Message::Close(_) => {
                return Err(AliyunRealtimeError::WebSocket(
                    "等待任务启动时连接已关闭".to_string(),
                ))
            }
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
            }
            _ => {}
        }
    }
}

fn collect_transcription_result(
    socket: &mut WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) -> Result<String, AliyunRealtimeError> {
    let mut segments = Vec::<Segment>::new();
    let mut sequence = 0u64;

    loop {
        let message = socket
            .read()
            .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
        match message {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|err| AliyunRealtimeError::Parse(err.to_string()))?;
                match handle_server_event(&value, &mut segments, &mut sequence)? {
                    ServerEventAction::TaskFinished => break,
                    ServerEventAction::Continue | ServerEventAction::TaskStarted => {}
                }
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                socket
                    .send(Message::Pong(payload))
                    .map_err(|err| AliyunRealtimeError::WebSocket(err.to_string()))?;
            }
            _ => {}
        }
    }

    if segments.is_empty() {
        return Ok(String::new());
    }

    segments.sort_by(|left, right| {
        let left_begin = left.begin_time.unwrap_or(u64::MAX);
        let right_begin = right.begin_time.unwrap_or(u64::MAX);
        left_begin
            .cmp(&right_begin)
            .then_with(|| left.order.cmp(&right.order))
    });

    Ok(segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string())
}

fn handle_server_event(
    value: &Value,
    segments: &mut Vec<Segment>,
    sequence: &mut u64,
) -> Result<ServerEventAction, AliyunRealtimeError> {
    let event = value
        .pointer("/header/event")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match event {
        "task-failed" => Err(AliyunRealtimeError::Request(extract_error_message(value))),
        "task-started" => Ok(ServerEventAction::TaskStarted),
        "task-finished" => Ok(ServerEventAction::TaskFinished),
        "result-generated" => {
            collect_segments(value, segments, sequence);
            Ok(ServerEventAction::Continue)
        }
        _ => Ok(ServerEventAction::Continue),
    }
}

fn collect_segments(value: &Value, segments: &mut Vec<Segment>, sequence: &mut u64) {
    let output = value.pointer("/payload/output").unwrap_or(&Value::Null);
    let sentence_value = output.get("sentence").unwrap_or(&Value::Null);
    if sentence_value.is_array() {
        if let Some(items) = sentence_value.as_array() {
            for item in items {
                push_segment(item, segments, sequence);
            }
        }
        return;
    }
    if sentence_value.is_object() {
        push_segment(sentence_value, segments, sequence);
        return;
    }
    if let Some(text) = sentence_value.as_str() {
        push_segment(
            &json!({
                "text": text
            }),
            segments,
            sequence,
        );
        return;
    }
    if let Some(text) = output.get("text").and_then(|v| v.as_str()) {
        push_segment(
            &json!({
                "text": text
            }),
            segments,
            sequence,
        );
    }
}

fn push_segment(value: &Value, segments: &mut Vec<Segment>, sequence: &mut u64) {
    let text = value
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        return;
    }

    let begin_time = value
        .get("begin_time")
        .and_then(parse_u64)
        .or_else(|| value.get("beginTime").and_then(parse_u64));
    if let Some(begin) = begin_time {
        if let Some(existing) = segments
            .iter_mut()
            .find(|segment| segment.begin_time == Some(begin))
        {
            existing.text = text.to_string();
            return;
        }
    }

    *sequence += 1;
    segments.push(Segment {
        begin_time,
        order: *sequence,
        text: text.to_string(),
    });
}

fn parse_u64(value: &Value) -> Option<u64> {
    if let Some(number) = value.as_u64() {
        return Some(number);
    }
    value.as_str().and_then(|text| text.parse::<u64>().ok())
}

fn extract_error_message(value: &Value) -> String {
    value
        .pointer("/header/error_message")
        .and_then(|v| v.as_str())
        .or_else(|| value.pointer("/header/errorMessage").and_then(|v| v.as_str()))
        .or_else(|| value.pointer("/payload/message").and_then(|v| v.as_str()))
        .unwrap_or("阿里云任务失败")
        .to_string()
}

fn read_wav_as_pcm16k_mono(path: &Path) -> Result<Vec<u8>, AliyunRealtimeError> {
    let bytes = fs::read(path).map_err(|err| AliyunRealtimeError::Io(err.to_string()))?;
    let cursor = std::io::Cursor::new(bytes);
    let mut reader = WavReader::new(cursor).map_err(|err| AliyunRealtimeError::Io(err.to_string()))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err(AliyunRealtimeError::Io("音频通道数无效".to_string()));
    }

    let samples = match spec.sample_format {
        SampleFormat::Int if spec.bits_per_sample <= 16 => reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| AliyunRealtimeError::Io(err.to_string()))?
            .into_iter()
            .map(|sample| sample as f32 / i16::MAX as f32)
            .collect::<Vec<_>>(),
        SampleFormat::Int => reader
            .samples::<i32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| AliyunRealtimeError::Io(err.to_string()))?
            .into_iter()
            .map(|sample| sample as f32 / i32::MAX as f32)
            .collect::<Vec<_>>(),
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| AliyunRealtimeError::Io(err.to_string()))?,
    };

    let mono = to_mono(&samples, spec.channels as usize);
    let resampled = resample_linear(&mono, spec.sample_rate, 16000);
    let mut output = Vec::with_capacity(resampled.len() * 2);
    for sample in resampled {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32) as i16;
        output.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(output)
}

fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect()
}

fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || source_rate == target_rate {
        return samples.to_vec();
    }
    let output_len = ((samples.len() as u64 * target_rate as u64) / source_rate as u64) as usize;
    if output_len == 0 {
        return Vec::new();
    }
    let ratio = source_rate as f64 / target_rate as f64;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let position = index as f64 * ratio;
        let left = position.floor() as usize;
        let right = (left + 1).min(samples.len().saturating_sub(1));
        let alpha = (position - left as f64) as f32;
        let value = samples[left] * (1.0 - alpha) + samples[right] * alpha;
        output.push(value);
    }
    output
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{timestamp:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_task_message_contains_required_payload_fields() {
        let settings = Settings::default();
        let raw = build_run_task_message(&settings, ProviderKind::FunAsr, "task-1");
        let value: Value = serde_json::from_str(&raw).expect("run-task json");

        assert_eq!(
            value.pointer("/payload/model").and_then(|v| v.as_str()),
            Some(MODEL_FUN_ASR_REALTIME_V2)
        );
        assert_eq!(value.pointer("/payload/input"), Some(&json!({})));
        assert!(value.get("parameters").is_none());
        assert!(value.pointer("/payload/parameters/model").is_none());
    }

    #[test]
    fn handle_server_event_success_sequence() {
        let mut segments = Vec::new();
        let mut sequence = 0u64;

        let started = json!({ "header": { "event": "task-started" } });
        let started_action = handle_server_event(&started, &mut segments, &mut sequence)
            .expect("task-started should succeed");
        assert!(matches!(started_action, ServerEventAction::TaskStarted));

        let generated = json!({
            "header": { "event": "result-generated" },
            "payload": { "output": { "sentence": { "text": "hello", "begin_time": 1 } } }
        });
        let generated_action = handle_server_event(&generated, &mut segments, &mut sequence)
            .expect("result-generated should succeed");
        assert!(matches!(generated_action, ServerEventAction::Continue));
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "hello");

        let finished = json!({ "header": { "event": "task-finished" } });
        let finished_action = handle_server_event(&finished, &mut segments, &mut sequence)
            .expect("task-finished should succeed");
        assert!(matches!(finished_action, ServerEventAction::TaskFinished));
    }

    #[test]
    fn handle_server_event_task_failed_before_started() {
        let mut segments = Vec::new();
        let mut sequence = 0u64;
        let failed = json!({
            "header": { "event": "task-failed" },
            "payload": { "message": "boom" }
        });

        let err = handle_server_event(&failed, &mut segments, &mut sequence)
            .expect_err("task-failed should return error");
        let message = err.to_string();
        assert!(message.contains("boom"));
    }
}
