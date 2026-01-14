//! 火山引擎语音识别 API 实现
//!
//! 支持两种识别模式：
//! - 录音文件识别 (HTTP POST)
//! - 流式识别 (WebSocket)

use crate::settings::{Settings, VolcengineSettings};
use base64::{engine::general_purpose, Engine as _};
use hound::WavReader;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use thiserror::Error;
use tungstenite::{connect, Message};
use url::Url;

/// 录音文件识别 API 端点
const FILE_ASR_URL: &str = "https://openspeech.bytedance.com/api/v1/auc";

/// 流式识别 WebSocket 端点
const STREAMING_ASR_URL: &str = "wss://openspeech.bytedance.com/api/v1/asr";

/// 录音文件识别业务集群
const FILE_CLUSTER: &str = "volcengine_input_common";

/// 流式识别业务集群
const STREAMING_CLUSTER: &str = "volcengine_streaming_common";

#[derive(Debug, Error)]
pub enum VolcengineError {
    #[error("火山引擎请求失败: {0}")]
    Request(String),
    #[error("火山引擎响应解析失败: {0}")]
    Parse(String),
    #[error("火山引擎配置缺失: {0}")]
    Config(String),
    #[error("无法读取音频: {0}")]
    Io(String),
    #[error("WebSocket 错误: {0}")]
    WebSocket(String),
}

/// 录音文件识别请求
#[derive(Serialize)]
struct FileAsrRequest {
    app: AppInfo,
    user: UserInfo,
    audio: AudioInfo,
    request: RequestInfo,
}

#[derive(Serialize)]
struct AppInfo {
    appid: String,
    cluster: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Serialize)]
struct UserInfo {
    uid: String,
}

#[derive(Serialize)]
struct AudioInfo {
    data: String,
    format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
}

#[derive(Serialize)]
struct RequestInfo {
    sequence: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

/// 录音文件识别响应
#[derive(Deserialize)]
struct FileAsrResponse {
    code: i32,
    message: String,
    #[serde(default)]
    result: Option<String>,
}

#[derive(Clone, Copy)]
struct AudioMetadata {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
}

/// 转写音频文件
///
/// 根据设置选择使用录音文件识别或流式识别
pub fn transcribe_audio(settings: &Settings, audio_path: &Path) -> Result<String, VolcengineError> {
    ensure_config(&settings.volcengine)?;

    if settings.volcengine.use_streaming {
        transcribe_streaming(settings, audio_path)
    } else {
        transcribe_file(settings, audio_path)
    }
}

/// 录音文件识别 (HTTP POST)
fn transcribe_file(settings: &Settings, audio_path: &Path) -> Result<String, VolcengineError> {
    let file_bytes = fs::read(audio_path).map_err(|e| VolcengineError::Io(e.to_string()))?;
    let audio_base64 = general_purpose::STANDARD.encode(&file_bytes);

    let (audio_format, audio_meta) = detect_audio_info(audio_path);

    let request = FileAsrRequest {
        app: AppInfo {
            appid: settings.volcengine.app_id.clone(),
            cluster: FILE_CLUSTER.to_string(),
            token: Some(settings.volcengine.access_token.clone()),
        },
        user: UserInfo {
            uid: "vtt-keyboard".to_string(),
        },
        audio: AudioInfo {
            data: audio_base64,
            format: audio_format,
            rate: audio_meta.map(|meta| meta.sample_rate),
            language: Some(settings.volcengine.language.clone()),
        },
        request: RequestInfo {
            sequence: 1,
            version: if settings.volcengine.use_fast {
                Some("fast".to_string())
            } else {
                None
            },
        },
    };

    let client = reqwest::blocking::Client::new();
    let response = client
        .post(FILE_ASR_URL)
        .header("Authorization", format!("Bearer;{}", settings.volcengine.access_token))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .map_err(|e| VolcengineError::Request(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(VolcengineError::Request(format!("{}: {}", status, body)));
    }

    let body = response
        .text()
        .map_err(|e| VolcengineError::Parse(e.to_string()))?;

    let asr_response: FileAsrResponse =
        serde_json::from_str(&body).map_err(|e| VolcengineError::Parse(e.to_string()))?;

    if asr_response.code != 0 {
        return Err(VolcengineError::Request(format!(
            "错误码 {}: {}",
            asr_response.code, asr_response.message
        )));
    }

    Ok(asr_response.result.unwrap_or_default())
}

/// 流式识别 (WebSocket)
fn transcribe_streaming(
    settings: &Settings,
    audio_path: &Path,
) -> Result<String, VolcengineError> {
    let file_bytes = fs::read(audio_path).map_err(|e| VolcengineError::Io(e.to_string()))?;
    let (audio_format, audio_meta) = detect_audio_info(audio_path);

    // 连接 WebSocket
    let url = Url::parse(STREAMING_ASR_URL).map_err(|e| VolcengineError::WebSocket(e.to_string()))?;
    let (mut socket, _response) =
        connect(url).map_err(|e| VolcengineError::WebSocket(e.to_string()))?;

    // 发送握手消息
    let handshake = build_streaming_handshake(settings, &audio_format, audio_meta);
    socket
        .send(Message::Text(handshake))
        .map_err(|e| VolcengineError::WebSocket(e.to_string()))?;

    // 等待握手响应
    let handshake_response = socket
        .read()
        .map_err(|e| VolcengineError::WebSocket(e.to_string()))?;

    if let Message::Text(text) = handshake_response {
        let resp: Value =
            serde_json::from_str(&text).map_err(|e| VolcengineError::Parse(e.to_string()))?;
        if resp.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) != 0 {
            let msg = resp
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("握手失败");
            return Err(VolcengineError::WebSocket(msg.to_string()));
        }
    }

    // 分块发送音频数据
    let chunk_size = compute_chunk_size(audio_meta);
    let mut offset = 0;
    let mut sequence = 1;

    while offset < file_bytes.len() {
        let end = (offset + chunk_size).min(file_bytes.len());
        let chunk = &file_bytes[offset..end];
        let is_last = end >= file_bytes.len();

        let audio_msg = build_audio_message(chunk, sequence, is_last);
        socket
            .send(Message::Text(audio_msg))
            .map_err(|e| VolcengineError::WebSocket(e.to_string()))?;

        offset = end;
        sequence += 1;
    }

    // 收集识别结果
    let mut final_text = String::new();
    loop {
        let msg = socket
            .read()
            .map_err(|e| VolcengineError::WebSocket(e.to_string()))?;

        match msg {
            Message::Text(text) => {
                let resp: Value = serde_json::from_str(&text)
                    .map_err(|e| VolcengineError::Parse(e.to_string()))?;

                // 检查是否有错误
                if let Some(code) = resp.get("code").and_then(|v| v.as_i64()) {
                    if code != 0 {
                        let msg = resp
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("识别错误");
                        return Err(VolcengineError::Request(msg.to_string()));
                    }
                }

                // 提取识别结果
                if let Some(result) = resp.get("result").and_then(|v| v.as_str()) {
                    final_text = result.to_string();
                }

                // 检查是否结束
                if resp.get("is_last").and_then(|v| v.as_bool()).unwrap_or(false) {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => continue,
        }
    }

    let _ = socket.close(None);
    Ok(final_text)
}

/// 构建流式识别握手消息
fn build_streaming_handshake(
    settings: &Settings,
    format: &str,
    metadata: Option<AudioMetadata>,
) -> String {
    let sample_rate = metadata.map(|meta| meta.sample_rate).unwrap_or(16000);
    let channels = metadata.map(|meta| meta.channels).unwrap_or(1);
    let bits = metadata.map(|meta| meta.bits_per_sample).unwrap_or(16);

    let handshake = json!({
        "app": {
            "appid": settings.volcengine.app_id,
            "cluster": STREAMING_CLUSTER,
            "token": settings.volcengine.access_token,
        },
        "user": {
            "uid": "vtt-keyboard"
        },
        "request": {
            "reqid": uuid_simple(),
            "workflow": "audio_in,resample,partition,vad,fe,decode,itn,nlu_punctuate",
            "sequence": 1,
            "nbest": 1,
            "show_utterances": true
        },
        "audio": {
            "format": format,
            "rate": sample_rate,
            "language": settings.volcengine.language,
            "bits": bits,
            "channel": channels,
            "codec": "raw"
        },
        "additions": {
            "use_fast": settings.volcengine.use_fast
        }
    });
    handshake.to_string()
}

/// 构建音频数据消息
fn build_audio_message(chunk: &[u8], sequence: i32, is_last: bool) -> String {
    let msg = json!({
        "audio": {
            "data": general_purpose::STANDARD.encode(chunk),
        },
        "request": {
            "sequence": sequence,
            "is_last": is_last
        }
    });
    msg.to_string()
}

/// 检测音频格式与元信息
fn detect_audio_info(path: &Path) -> (String, Option<AudioMetadata>) {
    let format = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_else(|| "wav".to_string());

    if format != "wav" {
        return (format, None);
    }

    let reader = match WavReader::open(path) {
        Ok(reader) => reader,
        Err(_) => return (format, None),
    };
    let spec = reader.spec();
    (
        format,
        Some(AudioMetadata {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
            bits_per_sample: spec.bits_per_sample,
        }),
    )
}

fn compute_chunk_size(metadata: Option<AudioMetadata>) -> usize {
    let Some(meta) = metadata else {
        return 3200;
    };
    let bits = meta.bits_per_sample as usize;
    let bytes_per_sample = ((bits + 7) / 8).max(1);
    let bytes_per_second = meta
        .sample_rate
        .saturating_mul(meta.channels as u32) as usize
        * bytes_per_sample;
    let chunk = bytes_per_second / 10;
    if chunk == 0 {
        3200
    } else {
        chunk
    }
}

/// 验证配置
fn ensure_config(settings: &VolcengineSettings) -> Result<(), VolcengineError> {
    if settings.app_id.trim().is_empty() {
        return Err(VolcengineError::Config("App ID 不能为空".to_string()));
    }
    if settings.access_token.trim().is_empty() {
        return Err(VolcengineError::Config("Access Token 不能为空".to_string()));
    }
    Ok(())
}

/// 生成简单的 UUID
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", timestamp)
}
