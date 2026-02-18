use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::Aead;
use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use thiserror::Error;
use url::Url;

const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_KEY_FILE: &str = "settings.key";
const SETTINGS_STORE_KEY: &str = "payload";

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("无法获取应用目录: {0}")]
    PathResolve(String),
    #[error("无法读写设置文件: {0}")]
    Io(String),
    #[error("无法处理加密数据: {0}")]
    Crypto(String),
    #[error("无法解析设置内容: {0}")]
    Serde(String),
    #[error("无法读取存储: {0}")]
    Store(String),
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub shortcut: ShortcutSettings,
    pub recording: RecordingSettings,
    #[serde(default)]
    pub provider: TranscriptionProvider,
    pub openai: OpenAiSettings,
    #[serde(default)]
    pub volcengine: VolcengineSettings,
    #[serde(default)]
    pub sensevoice: SenseVoiceSettings,
    #[serde(default = "default_triggers")]
    pub triggers: Vec<TriggerCard>,
    pub appearance: AppearanceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shortcut: ShortcutSettings {
                key: "CommandOrControl+Shift+Space".to_string(),
            },
            recording: RecordingSettings {
                segment_seconds: 60,
            },
            provider: TranscriptionProvider::default(),
            openai: OpenAiSettings {
                api_base: "https://api.openai.com/v1".to_string(),
                api_key: "".to_string(),
                speech_to_text: SpeechToTextSettings {
                    model: "gpt-4o-transcribe".to_string(),
                    language: "".to_string(),
                    prompt: "".to_string(),
                    response_format: "json".to_string(),
                    temperature: 0.0,
                    timestamp_granularities: vec![],
                    chunking_strategy: "auto".to_string(),
                    include: vec![],
                    stream: false,
                    known_speaker_names: vec![],
                    known_speaker_references: vec![],
                },
                text: TextSettings {
                    model: "gpt-4o-mini".to_string(),
                    temperature: 0.6,
                    max_output_tokens: 800,
                    top_p: 1.0,
                    instructions: "".to_string(),
                },
            },
            volcengine: VolcengineSettings::default(),
            sensevoice: SenseVoiceSettings::default(),
            triggers: default_triggers(),
            appearance: AppearanceSettings {
                theme: "system".to_string(),
            },
        }
    }
}

fn default_triggers() -> Vec<TriggerCard> {
    vec![
        TriggerCard {
            id: "translate".to_string(),
            title: "翻译".to_string(),
            enabled: true,
            auto_apply: false,
            locked: true,
            keyword: "翻译为{value}".to_string(),
            prompt_template: "请将以下内容翻译为{value}。".to_string(),
            variables: vec!["英文".to_string()],
        },
        TriggerCard {
            id: "polish".to_string(),
            title: "润色".to_string(),
            enabled: true,
            auto_apply: false,
            locked: true,
            keyword: "润色为{value}".to_string(),
            prompt_template: "请将以下内容润色为{value}。".to_string(),
            variables: vec!["口语".to_string()],
        },
    ]
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSettings {
    pub key: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSettings {
    pub segment_seconds: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAiSettings {
    pub api_base: String,
    pub api_key: String,
    pub speech_to_text: SpeechToTextSettings,
    pub text: TextSettings,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechToTextSettings {
    pub model: String,
    pub language: String,
    pub prompt: String,
    pub response_format: String,
    pub temperature: f32,
    pub timestamp_granularities: Vec<String>,
    pub chunking_strategy: String,
    pub include: Vec<String>,
    pub stream: bool,
    pub known_speaker_names: Vec<String>,
    pub known_speaker_references: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSettings {
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub top_p: f32,
    pub instructions: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerCard {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub auto_apply: bool,
    pub locked: bool,
    pub keyword: String,
    pub prompt_template: String,
    pub variables: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: String,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProvider {
    #[default]
    Openai,
    Volcengine,
    Sensevoice,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolcengineSettings {
    pub app_id: String,
    pub access_token: String,
    pub use_streaming: bool,
    pub use_fast: bool,
    pub language: String,
}

impl Default for VolcengineSettings {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            access_token: String::new(),
            use_streaming: false,
            use_fast: false,
            language: "zh-CN".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseVoiceSettings {
    pub enabled: bool,
    pub installed: bool,
    pub service_url: String,
    pub model_id: String,
    pub device: String,
    pub download_state: String,
    pub last_error: String,
}

impl Default for SenseVoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            installed: false,
            service_url: "http://127.0.0.1:8765".to_string(),
            model_id: "iic/SenseVoiceSmall".to_string(),
            device: "auto".to_string(),
            download_state: "idle".to_string(),
            last_error: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    app: AppHandle,
}

impl SettingsStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn load(&self) -> Result<Settings, SettingsError> {
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        let Some(payload) = store.get(SETTINGS_STORE_KEY) else {
            let settings = Settings::default();
            self.save(&settings)?;
            return Ok(settings);
        };
        let encoded = payload
            .as_str()
            .ok_or_else(|| SettingsError::Serde("设置内容格式异常".to_string()))?;
        let key = self.load_or_create_key()?;
        let decrypted = decrypt_payload(encoded, &key)?;
        match serde_json::from_str(&decrypted) {
            Ok(settings) => Ok(settings),
            Err(_) => {
                let settings = Settings::default();
                let _ = self.save(&settings);
                Ok(settings)
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        validate_settings(settings)?;
        self.persist_settings(settings)
    }

    pub fn load_sensevoice(&self) -> Result<SenseVoiceSettings, SettingsError> {
        let settings = self.load()?;
        Ok(settings.sensevoice)
    }

    pub fn save_sensevoice(&self, sensevoice: &SenseVoiceSettings) -> Result<(), SettingsError> {
        validate_sensevoice_settings(sensevoice)?;
        let mut settings = self.load()?;
        settings.sensevoice = sensevoice.clone();
        self.persist_settings(&settings)
    }

    fn persist_settings(&self, settings: &Settings) -> Result<(), SettingsError> {
        let json = serde_json::to_string(settings).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(SETTINGS_STORE_KEY.to_string(), serde_json::Value::String(encrypted));
        store
            .save()
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        Ok(())
    }

    fn load_or_create_key(&self) -> Result<[u8; 32], SettingsError> {
        let dir = self
            .app
            .path()
            .app_data_dir()
            .map_err(|err| SettingsError::PathResolve(err.to_string()))?;
        fs::create_dir_all(&dir).map_err(|err| SettingsError::Io(err.to_string()))?;
        let key_path = dir.join(SETTINGS_KEY_FILE);
        if key_path.exists() {
            let data = fs::read_to_string(&key_path).map_err(|err| SettingsError::Io(err.to_string()))?;
            let decoded = general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|err| SettingsError::Crypto(err.to_string()))?;
            return decoded
                .as_slice()
                .try_into()
                .map_err(|_| SettingsError::Crypto("密钥长度不正确".to_string()));
        }
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let encoded = general_purpose::STANDARD.encode(key);
        fs::write(&key_path, encoded).map_err(|err| SettingsError::Io(err.to_string()))?;
        Ok(key)
    }
}

fn validate_settings(settings: &Settings) -> Result<(), SettingsError> {
    let required = ["translate", "polish"];
    for id in required {
        let exists = settings
            .triggers
            .iter()
            .any(|card| card.id == id && card.locked);
        if !exists {
            return Err(SettingsError::Serde(format!(
                "必须保留内置触发词: {id}"
            )));
        }
    }

    for card in &settings.triggers {
        let has_value = card
            .variables
            .iter()
            .any(|value| !value.trim().is_empty());
        if !has_value {
            return Err(SettingsError::Serde(format!(
                "触发词变量范围不能为空: {}",
                card.title
            )));
        }
        if card.keyword.trim().is_empty() {
            return Err(SettingsError::Serde(format!(
                "触发词关键字不能为空: {}",
                card.title
            )));
        }
        if card.keyword.matches("{value}").count() != 1 {
            return Err(SettingsError::Serde(format!(
                "触发词关键字必须包含且只包含一个 {{value}}: {}",
                card.title
            )));
        }
    }

    validate_sensevoice_settings(&settings.sensevoice)?;
    Ok(())
}

fn validate_sensevoice_settings(sensevoice: &SenseVoiceSettings) -> Result<(), SettingsError> {
    if sensevoice.service_url.trim().is_empty() {
        return Err(SettingsError::Serde(
            "SenseVoice 服务地址不能为空".to_string(),
        ));
    }
    let parsed = Url::parse(sensevoice.service_url.trim())
        .map_err(|err| SettingsError::Serde(format!("SenseVoice 服务地址无效: {err}")))?;
    if parsed.host_str().is_none() {
        return Err(SettingsError::Serde(
            "SenseVoice 服务地址缺少主机名".to_string(),
        ));
    }
    if parsed.port_or_known_default().is_none() {
        return Err(SettingsError::Serde(
            "SenseVoice 服务地址缺少端口".to_string(),
        ));
    }
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(SettingsError::Serde(
            "SenseVoice 服务地址必须使用 http 或 https".to_string(),
        ));
    }
    if sensevoice.model_id.trim().is_empty() {
        return Err(SettingsError::Serde(
            "SenseVoice 模型 ID 不能为空".to_string(),
        ));
    }
    if !matches!(sensevoice.device.as_str(), "auto" | "cpu" | "cuda") {
        return Err(SettingsError::Serde(
            "SenseVoice 推理设备仅支持 auto/cpu/cuda".to_string(),
        ));
    }
    Ok(())
}

fn encrypt_payload(plain: &str, key: &[u8; 32]) -> Result<String, SettingsError> {
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher_text = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let mut combined = Vec::with_capacity(nonce_bytes.len() + cipher_text.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&cipher_text);
    Ok(general_purpose::STANDARD.encode(combined))
}

fn decrypt_payload(encoded: &str, key: &[u8; 32]) -> Result<String, SettingsError> {
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    if decoded.len() < 12 {
        return Err(SettingsError::Crypto("密文长度不足".to_string()));
    }
    let (nonce_bytes, cipher_text) = decoded.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher
        .decrypt(nonce, cipher_text)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    String::from_utf8(plain).map_err(|err| SettingsError::Crypto(err.to_string()))
}
