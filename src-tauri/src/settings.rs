use crate::sensevoice::model::supports_sherpa_onnx_target;
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
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
const HISTORY_STORE_KEY: &str = "transcriptionHistory";
const UPDATER_STATE_STORE_KEY: &str = "updaterState";
pub const MAX_TRANSCRIPTION_HISTORY_ITEMS: usize = 200;
const LOCAL_MODEL_SENSEVOICE: &str = "sensevoice";
const LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE: &str = "sherpa-onnx-sensevoice";
const LOCAL_MODEL_VOXTRAL: &str = "voxtral";
const LOCAL_MODEL_QWEN3_ASR: &str = "qwen3-asr";
const ALIYUN_REGION_BEIJING: &str = "beijing";
const ALIYUN_REGION_SINGAPORE: &str = "singapore";
const VOXTRAL_REQUIRED_DEVICE: &str = "cuda";
const QWEN3_ASR_REQUIRED_DEVICE: &str = "cuda";
const STOP_MODE_STOP: &str = "stop";
const STOP_MODE_PAUSE: &str = "pause";
const DEFAULT_SENSEVOICE_MODEL_ID: &str = "FunAudioLLM/SenseVoiceSmall";
const DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID: &str =
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
const DEFAULT_VOXTRAL_MODEL_ID: &str = "mistralai/Voxtral-Mini-4B-Realtime-2602";
const DEFAULT_QWEN3_ASR_MODEL_ID: &str = "Qwen/Qwen3-ASR-1.7B";
const QWEN3_ASR_ALLOWED_MODEL_IDS: [&str; 3] = [
    "Qwen/Qwen3-ASR-1.7B",
    "Qwen/Qwen3-ASR-0.6B",
    "Qwen/Qwen3-ForcedAligner-0.6B",
];

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("闂佸搫鍟版慨鐢垫兜閸洘鍤旂€瑰嫭婢樼徊鍧楀箹鐎涙ɑ鈷掗柡浣靛€濋幆鍕敊閼测晝协: {0}")]
    PathResolve(String),
    #[error(
        "闂佸搫鍟版慨鐢垫兜閸撲焦瀚氶悹鍥ㄥ絻閺呮悂鎮规担绋库挃闁汇倕妫濆顒勫炊閿旂瓔鍋? {0}"
    )]
    Io(String),
    #[error("闂佸搫鍟版慨鐢垫兜閼搁潧绶為柛鏇ㄥ幗閸婄偤鏌涢弮鍌毿㈤柣锔藉灴瀵偊鎮ч崼婵堛偊: {0}")]
    Crypto(String),
    #[error("闂佸搫鍟版慨鐢垫兜閸撲焦鍠嗛柨婵嗘閳ь剛鏅幏瀣礈瑜忛弸鍌炴煕閹邦剚鍣规い? {0}")]
    Serde(String),
    #[error("闂佸搫鍟版慨鐢垫兜閸撲焦瀚氶悹鍥ㄥ絻缁插潡鎮楀☉娅亪宕? {0}")]
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
    pub text_processing: TextProcessingSettings,
    #[serde(default)]
    pub volcengine: VolcengineSettings,
    #[serde(default)]
    pub sensevoice: SenseVoiceSettings,
    #[serde(default)]
    pub aliyun: AliyunSettings,
    #[serde(default = "default_triggers")]
    pub triggers: Vec<TriggerCard>,
    #[serde(default)]
    pub output: OutputSettings,
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub startup: StartupSettings,
    #[serde(default)]
    pub history: HistorySettings,
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
                api_base: default_openai_api_base(),
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
                legacy_text: None,
            },
            text_processing: TextProcessingSettings::default(),
            volcengine: VolcengineSettings::default(),
            sensevoice: SenseVoiceSettings::default(),
            aliyun: AliyunSettings::default(),
            triggers: default_triggers(),
            output: OutputSettings::default(),
            appearance: AppearanceSettings {
                theme: "system".to_string(),
            },
            startup: StartupSettings::default(),
            history: HistorySettings::default(),
        }
    }
}

fn default_triggers() -> Vec<TriggerCard> {
    vec![
        TriggerCard {
            id: "translate".to_string(),
            title: "Translate".to_string(),
            enabled: true,
            auto_apply: false,
            locked: true,
            keyword: "translate".to_string(),
            prompt_template: "Translate the following content to {value}.".to_string(),
            variables: vec!["English".to_string()],
        },
        TriggerCard {
            id: "polish".to_string(),
            title: "Polish".to_string(),
            enabled: true,
            auto_apply: false,
            locked: true,
            keyword: "polish".to_string(),
            prompt_template: "Polish the following content into {value}.".to_string(),
            variables: vec!["spoken style".to_string()],
        },
    ]
}

fn default_openai_api_base() -> String {
    "https://api.openai.com/v1".to_string()
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
    #[serde(default, rename = "text", skip_serializing)]
    pub legacy_text: Option<TextSettings>,
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
pub struct TextProcessingSettings {
    #[serde(default)]
    pub provider: TextProcessingProvider,
    #[serde(default)]
    pub openai: TextSettings,
}

impl Default for TextProcessingSettings {
    fn default() -> Self {
        Self {
            provider: TextProcessingProvider::default(),
            openai: TextSettings::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSettings {
    #[serde(default = "default_openai_api_base")]
    pub api_base: String,
    #[serde(default)]
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub top_p: f32,
    pub instructions: String,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            api_base: default_openai_api_base(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.6,
            max_output_tokens: 800,
            top_p: 1.0,
            instructions: String::new(),
        }
    }
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
pub struct OutputSettings {
    pub remove_newlines: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            remove_newlines: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub theme: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSettings {
    pub launch_on_boot: bool,
    #[serde(default = "default_auto_check_updates")]
    pub auto_check_updates: bool,
    #[serde(default = "default_auto_install_updates_on_quit")]
    pub auto_install_updates_on_quit: bool,
}

impl Default for StartupSettings {
    fn default() -> Self {
        Self {
            launch_on_boot: false,
            auto_check_updates: default_auto_check_updates(),
            auto_install_updates_on_quit: default_auto_install_updates_on_quit(),
        }
    }
}

fn default_auto_check_updates() -> bool {
    true
}

fn default_auto_install_updates_on_quit() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySettings {
    pub enabled: bool,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerMatch {
    pub trigger_id: String,
    pub trigger_title: String,
    pub keyword: String,
    pub matched_value: String,
    pub mode: TriggerMatchMode,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerMatchMode {
    Keyword,
    Auto,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionHistoryStatus {
    Success,
    Failed,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionHistoryItem {
    pub id: String,
    pub timestamp_ms: u64,
    pub status: TranscriptionHistoryStatus,
    pub transcription_text: String,
    pub final_text: String,
    #[serde(default)]
    pub model_group: String,
    #[serde(default)]
    pub transcription_elapsed_ms: u64,
    #[serde(default)]
    pub recording_duration_ms: u64,
    pub triggered: bool,
    pub triggered_by_keyword: bool,
    pub trigger_matches: Vec<TriggerMatch>,
    #[serde(default)]
    pub alignment: Option<TranscriptionAlignment>,
    pub error_message: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionAlignment {
    pub tokens: Vec<String>,
    pub timestamps_ms: Vec<u64>,
    #[serde(default)]
    pub durations_ms: Vec<u64>,
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionProvider {
    #[default]
    Openai,
    Volcengine,
    Sensevoice,
    #[serde(rename = "aliyun-asr")]
    AliyunAsr,
    #[serde(rename = "aliyun-paraformer")]
    AliyunParaformer,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextProcessingProvider {
    #[default]
    Openai,
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
    #[serde(default = "default_local_model")]
    pub local_model: String,
    #[serde(default = "default_stop_mode")]
    pub stop_mode: String,
    pub service_url: String,
    pub model_id: String,
    #[serde(default = "default_sensevoice_language")]
    pub language: String,
    pub device: String,
    pub download_state: String,
    pub last_error: String,
}

fn default_local_model() -> String {
    "sensevoice".to_string()
}

fn default_stop_mode() -> String {
    STOP_MODE_STOP.to_string()
}

fn default_sensevoice_language() -> String {
    "auto".to_string()
}

impl Default for SenseVoiceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            installed: false,
            local_model: default_local_model(),
            stop_mode: default_stop_mode(),
            service_url: "http://127.0.0.1:28765".to_string(),
            model_id: DEFAULT_SENSEVOICE_MODEL_ID.to_string(),
            language: default_sensevoice_language(),
            device: "auto".to_string(),
            download_state: "idle".to_string(),
            last_error: String::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunSettings {
    #[serde(default = "default_aliyun_region")]
    pub region: String,
    #[serde(default)]
    pub api_keys: AliyunApiKeys,
    #[serde(default)]
    pub asr: AliyunAsrSettings,
    #[serde(default)]
    pub paraformer: AliyunParaformerSettings,
}

fn default_aliyun_region() -> String {
    ALIYUN_REGION_BEIJING.to_string()
}

impl Default for AliyunSettings {
    fn default() -> Self {
        Self {
            region: default_aliyun_region(),
            api_keys: AliyunApiKeys::default(),
            asr: AliyunAsrSettings::default(),
            paraformer: AliyunParaformerSettings::default(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunApiKeys {
    #[serde(default)]
    pub beijing: String,
    #[serde(default)]
    pub singapore: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunAsrSettings {
    #[serde(default)]
    pub vocabulary_id: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunParaformerSettings {
    #[serde(default)]
    pub language_hints: Vec<String>,
    #[serde(default)]
    pub vocabulary_id: String,
}

#[derive(Clone)]
pub struct SettingsStore {
    app: AppHandle,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdaterState {
    #[serde(default)]
    deferred_version: Option<String>,
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
        let encoded = payload.as_str().ok_or_else(|| {
            SettingsError::Serde("settings payload has invalid format".to_string())
        })?;
        let key = self.load_or_create_key()?;
        let decrypted = decrypt_payload(encoded, &key)?;
        match serde_json::from_str::<Settings>(&decrypted) {
            Ok(mut settings) => {
                normalize_sensevoice_settings(&mut settings.sensevoice);
                normalize_aliyun_settings(&mut settings.aliyun, &settings.provider);
                normalize_text_processing_settings(&mut settings);
                Ok(settings)
            }
            Err(_) => {
                let settings = Settings::default();
                let _ = self.save(&settings);
                Ok(settings)
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let mut normalized = settings.clone();
        normalize_sensevoice_settings(&mut normalized.sensevoice);
        normalize_aliyun_settings(&mut normalized.aliyun, &normalized.provider);
        normalize_text_processing_settings(&mut normalized);
        validate_settings(&normalized)?;
        self.persist_settings(&normalized)
    }

    pub fn load_sensevoice(&self) -> Result<SenseVoiceSettings, SettingsError> {
        let settings = self.load()?;
        Ok(settings.sensevoice)
    }

    pub fn save_sensevoice(&self, sensevoice: &SenseVoiceSettings) -> Result<(), SettingsError> {
        let mut normalized = sensevoice.clone();
        normalize_sensevoice_settings(&mut normalized);
        validate_sensevoice_settings(&normalized)?;
        let mut settings = self.load()?;
        settings.sensevoice = normalized;
        self.persist_settings(&settings)
    }

    pub fn save_sensevoice_editable(
        &self,
        sensevoice: &SenseVoiceSettings,
    ) -> Result<(), SettingsError> {
        let mut settings = self.load()?;
        let mut merged = settings.sensevoice.clone();
        merged.local_model = sensevoice.local_model.clone();
        merged.stop_mode = sensevoice.stop_mode.clone();
        merged.service_url = sensevoice.service_url.clone();
        merged.model_id = sensevoice.model_id.clone();
        merged.language = sensevoice.language.clone();
        merged.device = sensevoice.device.clone();
        normalize_sensevoice_settings(&mut merged);
        validate_sensevoice_settings(&merged)?;
        // Runtime-managed fields are preserved from the persisted settings and must not
        // be overwritten by the editable settings payload coming from the UI.
        settings.sensevoice = merged;
        self.persist_settings(&settings)
    }

    pub fn load_transcription_history(
        &self,
    ) -> Result<Vec<TranscriptionHistoryItem>, SettingsError> {
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        let Some(payload) = store.get(HISTORY_STORE_KEY) else {
            return Ok(Vec::new());
        };
        let Some(encoded) = payload.as_str() else {
            return Ok(Vec::new());
        };
        let key = self.load_or_create_key()?;
        let decrypted = match decrypt_payload(encoded, &key) {
            Ok(value) => value,
            Err(_) => return Ok(Vec::new()),
        };
        match serde_json::from_str::<Vec<TranscriptionHistoryItem>>(&decrypted) {
            Ok(mut history) => {
                if history.len() > MAX_TRANSCRIPTION_HISTORY_ITEMS {
                    history.truncate(MAX_TRANSCRIPTION_HISTORY_ITEMS);
                }
                Ok(history)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    pub fn append_transcription_history(
        &self,
        item: TranscriptionHistoryItem,
    ) -> Result<(), SettingsError> {
        let mut history = self.load_transcription_history()?;
        history.insert(0, item);
        if history.len() > MAX_TRANSCRIPTION_HISTORY_ITEMS {
            history.truncate(MAX_TRANSCRIPTION_HISTORY_ITEMS);
        }
        self.persist_transcription_history(&history)
    }

    pub fn clear_transcription_history(&self) -> Result<(), SettingsError> {
        self.persist_transcription_history(&[])
    }

    pub fn load_deferred_update_version(&self) -> Result<Option<String>, SettingsError> {
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        let Some(payload) = store.get(UPDATER_STATE_STORE_KEY) else {
            return Ok(None);
        };
        let Some(encoded) = payload.as_str() else {
            return Ok(None);
        };
        let key = self.load_or_create_key()?;
        let decrypted = match decrypt_payload(encoded, &key) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        let state = match serde_json::from_str::<UpdaterState>(&decrypted) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        Ok(state
            .deferred_version
            .filter(|value| !value.trim().is_empty()))
    }

    pub fn save_deferred_update_version(&self, version: Option<&str>) -> Result<(), SettingsError> {
        let deferred_version = version
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let state = UpdaterState { deferred_version };
        let json =
            serde_json::to_string(&state).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(
            UPDATER_STATE_STORE_KEY.to_string(),
            serde_json::Value::String(encrypted),
        );
        store
            .save()
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        Ok(())
    }

    fn persist_settings(&self, settings: &Settings) -> Result<(), SettingsError> {
        let json =
            serde_json::to_string(settings).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(
            SETTINGS_STORE_KEY.to_string(),
            serde_json::Value::String(encrypted),
        );
        store
            .save()
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        Ok(())
    }

    fn persist_transcription_history(
        &self,
        history: &[TranscriptionHistoryItem],
    ) -> Result<(), SettingsError> {
        let json =
            serde_json::to_string(history).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(
            HISTORY_STORE_KEY.to_string(),
            serde_json::Value::String(encrypted),
        );
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
            let data =
                fs::read_to_string(&key_path).map_err(|err| SettingsError::Io(err.to_string()))?;
            let decoded = general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|err| SettingsError::Crypto(err.to_string()))?;
            return decoded
                .as_slice()
                .try_into()
                .map_err(|_| SettingsError::Crypto("invalid key length".to_string()));
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
            return Err(SettingsError::Serde(format!("闂婎偄娲ら幊姗€濡磋箛鏃傗攳婵犻潧鐗婂▓宀勬煕閹邦剚鍣介柣銈呮閹叉挳鏁冮埀顒冦亹閸屾粍瀚? {id}")));
        }
    }

    for card in &settings.triggers {
        let has_value = card.variables.iter().any(|value| !value.trim().is_empty());
        if !has_value {
            return Err(SettingsError::Serde(format!(
                "闁荤喐鐟辩粻鎴ｃ亹閸屾粍瀚氱€广儱鎳庣紞渚€姊洪幓鎺旂閻庡灚鐗犲畷鍓佺礄閻樼數鎲归梺鑹版珪閸ㄦ繄鎷归悢铏圭煔? {}",
                card.title
            )));
        }
        if card.keyword.trim().is_empty() {
            return Err(SettingsError::Serde(format!(
                "闁荤喐鐟辩粻鎴ｃ亹閸屾粍瀚氱€广儱鎳庤ぐ鐘绘⒑濞嗘儳鏋涢柣鈯欏嫮鈻旂€广儱鐗嗛崢鏉戔槈閹捐顏犻柍? {}",
                card.title
            )));
        }
        if card.keyword.matches("{value}").count() > 1 {
            return Err(SettingsError::Serde(format!(
                "闁荤喐鐟辩粻鎴ｃ亹閸屾粍瀚氱€广儱鎳庤ぐ鐘绘⒑濞嗘儳鏋涢柣鈯欏洤瀚夐柍褜鍓氬鍕潩椤掆偓閻﹀爼鏌涘鐓庝簵缂佹柨鐡ㄧ粙?{{value}}: {}",
                card.title
            )));
        }
    }

    validate_sensevoice_settings(&settings.sensevoice)?;
    validate_aliyun_settings(settings)?;
    Ok(())
}

fn normalize_text_processing_settings(settings: &mut Settings) {
    settings.text_processing.provider = TextProcessingProvider::Openai;

    if settings.text_processing.openai.api_base.trim().is_empty() {
        settings.text_processing.openai.api_base = settings.openai.api_base.clone();
    }
    if settings.text_processing.openai.api_base.trim().is_empty() {
        settings.text_processing.openai.api_base = default_openai_api_base();
    }
    if settings.text_processing.openai.api_key.trim().is_empty() {
        settings.text_processing.openai.api_key = settings.openai.api_key.clone();
    }

    if let Some(legacy_text) = settings.openai.legacy_text.take() {
        settings.text_processing.openai.api_base = if settings.openai.api_base.trim().is_empty() {
            default_openai_api_base()
        } else {
            settings.openai.api_base.clone()
        };
        settings.text_processing.openai.api_key = settings.openai.api_key.clone();
        settings.text_processing.openai.model = legacy_text.model;
        settings.text_processing.openai.temperature = legacy_text.temperature;
        settings.text_processing.openai.max_output_tokens = legacy_text.max_output_tokens;
        settings.text_processing.openai.top_p = legacy_text.top_p;
        settings.text_processing.openai.instructions = legacy_text.instructions;
    }
}

fn normalize_sensevoice_settings(sensevoice: &mut SenseVoiceSettings) {
    sensevoice.stop_mode = normalize_stop_mode(&sensevoice.stop_mode).to_string();
    if sensevoice
        .local_model
        .eq_ignore_ascii_case(LOCAL_MODEL_VOXTRAL)
    {
        sensevoice.local_model = LOCAL_MODEL_VOXTRAL.to_string();
        sensevoice.device = VOXTRAL_REQUIRED_DEVICE.to_string();
        sensevoice.model_id = DEFAULT_VOXTRAL_MODEL_ID.to_string();
        sensevoice.language = default_sensevoice_language();
        return;
    }
    if sensevoice
        .local_model
        .eq_ignore_ascii_case(LOCAL_MODEL_QWEN3_ASR)
    {
        sensevoice.local_model = LOCAL_MODEL_QWEN3_ASR.to_string();
        sensevoice.device = QWEN3_ASR_REQUIRED_DEVICE.to_string();
        sensevoice.model_id = normalize_qwen3_asr_model_id(&sensevoice.model_id).to_string();
        sensevoice.language = default_sensevoice_language();
        return;
    }
    if sensevoice
        .local_model
        .eq_ignore_ascii_case(LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE)
    {
        if !supports_sherpa_onnx_target() {
            sensevoice.local_model = LOCAL_MODEL_SENSEVOICE.to_string();
            sensevoice.model_id = DEFAULT_SENSEVOICE_MODEL_ID.to_string();
            sensevoice.language = default_sensevoice_language();
            return;
        }
        sensevoice.local_model = LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE.to_string();
        sensevoice.device = "cpu".to_string();
        sensevoice.model_id = DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID.to_string();
        sensevoice.language = normalize_sensevoice_language(&sensevoice.language).to_string();
        return;
    }
    sensevoice.local_model = LOCAL_MODEL_SENSEVOICE.to_string();
    sensevoice.model_id = DEFAULT_SENSEVOICE_MODEL_ID.to_string();
    sensevoice.language = default_sensevoice_language();
}

fn normalize_aliyun_settings(aliyun: &mut AliyunSettings, provider: &TranscriptionProvider) {
    aliyun.region = normalize_aliyun_region(&aliyun.region).to_string();
    if matches!(provider, TranscriptionProvider::AliyunParaformer) {
        aliyun.region = ALIYUN_REGION_BEIJING.to_string();
    }
    aliyun.api_keys.beijing = aliyun.api_keys.beijing.trim().to_string();
    aliyun.api_keys.singapore = aliyun.api_keys.singapore.trim().to_string();
    aliyun.asr.vocabulary_id = aliyun.asr.vocabulary_id.trim().to_string();
    aliyun.paraformer.vocabulary_id = aliyun.paraformer.vocabulary_id.trim().to_string();
    aliyun.paraformer.language_hints = aliyun
        .paraformer
        .language_hints
        .iter()
        .map(|hint| hint.trim())
        .filter(|hint| !hint.is_empty())
        .map(ToString::to_string)
        .collect();
}

fn normalize_aliyun_region(region: &str) -> &str {
    if region.eq_ignore_ascii_case(ALIYUN_REGION_SINGAPORE) {
        ALIYUN_REGION_SINGAPORE
    } else {
        ALIYUN_REGION_BEIJING
    }
}

fn normalize_stop_mode(mode: &str) -> &str {
    if mode.eq_ignore_ascii_case(STOP_MODE_PAUSE) {
        STOP_MODE_PAUSE
    } else {
        STOP_MODE_STOP
    }
}

fn normalize_qwen3_asr_model_id(model_id: &str) -> &str {
    let trimmed = model_id.trim();
    QWEN3_ASR_ALLOWED_MODEL_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == trimmed)
        .unwrap_or(DEFAULT_QWEN3_ASR_MODEL_ID)
}

fn normalize_sensevoice_language(language: &str) -> &str {
    let trimmed = language.trim();
    if matches!(trimmed, "zh" | "en" | "ja" | "ko" | "yue") {
        trimmed
    } else {
        "auto"
    }
}

fn validate_aliyun_settings(settings: &Settings) -> Result<(), SettingsError> {
    let region = settings.aliyun.region.as_str();
    if !matches!(region, ALIYUN_REGION_BEIJING | ALIYUN_REGION_SINGAPORE) {
        return Err(SettingsError::Serde(
            "闂傚倸鍟锟犲闯闁垮顩查柟瀛樼箖閸曢箖鏌涢埡鍐ㄦ瀺缂侇喖閰ｅ銊╊敍濞戞妲?beijing/singapore".to_string(),
        ));
    }

    if matches!(settings.provider, TranscriptionProvider::AliyunParaformer)
        && region != ALIYUN_REGION_BEIJING
    {
        return Err(SettingsError::Serde(
            "Paraformer only supports beijing region".to_string(),
        ));
    }

    if matches!(
        settings.provider,
        TranscriptionProvider::AliyunAsr | TranscriptionProvider::AliyunParaformer
    ) {
        let api_key = if region == ALIYUN_REGION_SINGAPORE {
            settings.aliyun.api_keys.singapore.trim()
        } else {
            settings.aliyun.api_keys.beijing.trim()
        };
        if api_key.is_empty() {
            return Err(SettingsError::Serde(
                "Aliyun API Key for the selected region cannot be empty".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_sensevoice_settings(sensevoice: &SenseVoiceSettings) -> Result<(), SettingsError> {
    if !matches!(
        sensevoice.local_model.as_str(),
        LOCAL_MODEL_SENSEVOICE
            | LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE
            | LOCAL_MODEL_VOXTRAL
            | LOCAL_MODEL_QWEN3_ASR
    ) {
        return Err(SettingsError::Serde(
            "SenseVoice local model must be one of: sensevoice/sherpa-onnx-sensevoice/voxtral/qwen3-asr".to_string(),
        ));
    }
    if sensevoice.local_model != LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE {
        if sensevoice.service_url.trim().is_empty() {
            return Err(SettingsError::Serde(
                "SenseVoice service URL cannot be empty".to_string(),
            ));
        }
        let parsed = Url::parse(sensevoice.service_url.trim()).map_err(|err| {
            SettingsError::Serde(format!(
                "SenseVoice 闂佸搫鐗嗙粔瀛樻叏閻旂厧鎹堕柡澶嬪缁插鏌￠崘顏勑ｉ柡? {err}"
            ))
        })?;
        if parsed.host_str().is_none() {
            return Err(SettingsError::Serde(
                "SenseVoice service URL must include a host".to_string(),
            ));
        }
        if parsed.port_or_known_default().is_none() {
            return Err(SettingsError::Serde(
                "SenseVoice service URL must include a port".to_string(),
            ));
        }
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(SettingsError::Serde(
                "SenseVoice 闂佸搫鐗嗙粔瀛樻叏閻旂厧鎹堕柡澶嬪缁插鐓崶褎鍤囬柕鍡楃箲閹峰懐鎹勯妸锔芥 http 闂?https".to_string(),
            ));
        }
    }
    if sensevoice.model_id.trim().is_empty() {
        return Err(SettingsError::Serde(
            "SenseVoice model ID cannot be empty".to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_SENSEVOICE
        && sensevoice.model_id != DEFAULT_SENSEVOICE_MODEL_ID
    {
        return Err(SettingsError::Serde(
            "SenseVoice model ID must use the default value".to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_VOXTRAL
        && sensevoice.model_id != DEFAULT_VOXTRAL_MODEL_ID
    {
        return Err(SettingsError::Serde(
            "Voxtral model ID must use the default value".to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_QWEN3_ASR
        && !QWEN3_ASR_ALLOWED_MODEL_IDS.contains(&sensevoice.model_id.as_str())
    {
        return Err(SettingsError::Serde(
            "Qwen3-ASR model ID only supports preset values".to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE
        && sensevoice.model_id != DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID
    {
        return Err(SettingsError::Serde(
            "Sherpa-ONNX SenseVoice model ID must use the default value".to_string(),
        ));
    }
    if !matches!(
        sensevoice.language.as_str(),
        "auto" | "zh" | "en" | "ja" | "ko" | "yue"
    ) {
        return Err(SettingsError::Serde(
            "SenseVoice language only supports auto/zh/en/ja/ko/yue".to_string(),
        ));
    }
    if !matches!(sensevoice.device.as_str(), "auto" | "cpu" | "cuda") {
        return Err(SettingsError::Serde(
            "SenseVoice 闂佽浜介崝搴ㄥ箖婵犲嫭濯奸柟顖嗗本校婵炲濮撮幊蹇涘极椤曗偓楠?auto/cpu/cuda".to_string(),
        ));
    }
    if !matches!(
        sensevoice.stop_mode.as_str(),
        STOP_MODE_STOP | STOP_MODE_PAUSE
    ) {
        return Err(SettingsError::Serde(
            "SenseVoice 闂佺顑嗙划宥夘敆濞戞鐔煎灳瀹曞洨顢呮繛瀵稿Т閹冲繘寮鈧獮?stop/pause"
                .to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_VOXTRAL && sensevoice.device != VOXTRAL_REQUIRED_DEVICE
    {
        return Err(SettingsError::Serde(
            "Voxtral only supports CUDA devices".to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_QWEN3_ASR
        && sensevoice.device != QWEN3_ASR_REQUIRED_DEVICE
    {
        return Err(SettingsError::Serde(
            "Qwen3-ASR only supports CUDA devices".to_string(),
        ));
    }
    if sensevoice.local_model == LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE && sensevoice.device != "cpu" {
        return Err(SettingsError::Serde(
            "Sherpa-ONNX SenseVoice currently runs on CPU only".to_string(),
        ));
    }
    Ok(())
}

fn encrypt_payload(plain: &str, key: &[u8; 32]) -> Result<String, SettingsError> {
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|err| SettingsError::Crypto(err.to_string()))?;
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
        return Err(SettingsError::Crypto("cipher text too short".to_string()));
    }
    let (nonce_bytes, cipher_text) = decoded.split_at(12);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher
        .decrypt(nonce, cipher_text)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    String::from_utf8(plain).map_err(|err| SettingsError::Crypto(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_text_processing_settings, default_openai_api_base, OpenAiSettings, Settings,
        SpeechToTextSettings, TextProcessingProvider, TextSettings,
    };

    #[test]
    fn normalize_text_processing_migrates_legacy_openai_text_settings() {
        let mut settings = Settings::default();
        settings.openai = OpenAiSettings {
            api_base: "https://legacy.example/v1".to_string(),
            api_key: "legacy-key".to_string(),
            speech_to_text: SpeechToTextSettings {
                model: "gpt-4o-transcribe".to_string(),
                ..settings.openai.speech_to_text.clone()
            },
            legacy_text: Some(TextSettings {
                api_base: default_openai_api_base(),
                api_key: String::new(),
                model: "gpt-4.1-mini".to_string(),
                temperature: 0.2,
                max_output_tokens: 256,
                top_p: 0.8,
                instructions: "Rewrite the text".to_string(),
            }),
        };

        normalize_text_processing_settings(&mut settings);

        assert_eq!(settings.text_processing.provider, TextProcessingProvider::Openai);
        assert_eq!(
            settings.text_processing.openai.api_base,
            "https://legacy.example/v1"
        );
        assert_eq!(settings.text_processing.openai.api_key, "legacy-key");
        assert_eq!(settings.text_processing.openai.model, "gpt-4.1-mini");
        assert_eq!(settings.text_processing.openai.temperature, 0.2);
        assert_eq!(settings.text_processing.openai.max_output_tokens, 256);
        assert_eq!(settings.text_processing.openai.top_p, 0.8);
        assert_eq!(
            settings.text_processing.openai.instructions,
            "Rewrite the text"
        );
        assert!(settings.openai.legacy_text.is_none());
    }

    #[test]
    fn normalize_text_processing_backfills_auth_from_transcription_openai_settings() {
        let mut settings = Settings::default();
        settings.openai.api_base = "https://api.proxy/v1".to_string();
        settings.openai.api_key = "shared-key".to_string();
        settings.text_processing.openai.api_base.clear();
        settings.text_processing.openai.api_key.clear();

        normalize_text_processing_settings(&mut settings);

        assert_eq!(settings.text_processing.provider, TextProcessingProvider::Openai);
        assert_eq!(settings.text_processing.openai.api_base, "https://api.proxy/v1");
        assert_eq!(settings.text_processing.openai.api_key, "shared-key");
    }
}
