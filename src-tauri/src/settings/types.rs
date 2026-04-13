use serde::{Deserialize, Serialize};

pub const MAX_TRANSCRIPTION_HISTORY_ITEMS: usize = 200;
pub(crate) const LOCAL_MODEL_SENSEVOICE: &str = "sensevoice";
pub(crate) const LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE: &str = "sherpa-onnx-sensevoice";
pub(crate) const LOCAL_MODEL_VOXTRAL: &str = "voxtral";
pub(crate) const LOCAL_MODEL_QWEN3_ASR: &str = "qwen3-asr";
pub(crate) const ALIYUN_REGION_BEIJING: &str = "beijing";
pub(crate) const ALIYUN_REGION_SINGAPORE: &str = "singapore";
pub(crate) const VOXTRAL_REQUIRED_DEVICE: &str = "cuda";
pub(crate) const QWEN3_ASR_REQUIRED_DEVICE: &str = "cuda";
pub(crate) const STOP_MODE_STOP: &str = "stop";
pub(crate) const STOP_MODE_PAUSE: &str = "pause";
pub(crate) const DEFAULT_SENSEVOICE_MODEL_ID: &str = "FunAudioLLM/SenseVoiceSmall";
pub(crate) const DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID: &str =
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
pub(crate) const DEFAULT_VOXTRAL_MODEL_ID: &str = "mistralai/Voxtral-Mini-4B-Realtime-2602";
pub(crate) const DEFAULT_QWEN3_ASR_MODEL_ID: &str = "Qwen/Qwen3-ASR-1.7B";
pub(crate) const QWEN3_ASR_ALLOWED_MODEL_IDS: [&str; 3] = [
    "Qwen/Qwen3-ASR-1.7B",
    "Qwen/Qwen3-ASR-0.6B",
    "Qwen/Qwen3-ForcedAligner-0.6B",
];

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

pub(crate) fn default_openai_api_base() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_text_model() -> String {
    "gpt-4o-mini".to_string()
}

fn default_text_temperature() -> f32 {
    0.6
}

fn default_text_max_output_tokens() -> u32 {
    800
}

fn default_text_top_p() -> f32 {
    1.0
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
    #[serde(default = "default_text_model")]
    pub model: String,
    #[serde(default = "default_text_temperature")]
    pub temperature: f32,
    #[serde(default = "default_text_max_output_tokens")]
    pub max_output_tokens: u32,
    #[serde(default = "default_text_top_p")]
    pub top_p: f32,
    #[serde(default)]
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

// ── 提供商分层分类 ────────────────────────────────────────────

/// 提供商大类：云端 / 本地
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderCategory {
    Cloud,
    Local,
}

impl TranscriptionProvider {
    /// 返回提供商所属的大类
    pub fn category(&self) -> ProviderCategory {
        match self {
            Self::Sensevoice => ProviderCategory::Local,
            Self::Openai
            | Self::Volcengine
            | Self::AliyunAsr
            | Self::AliyunParaformer => ProviderCategory::Cloud,
        }
    }

    /// 判断是否为本地提供商
    pub fn is_local(&self) -> bool {
        self.category() == ProviderCategory::Local
    }

    /// 判断是否为云端提供商
    pub fn is_cloud(&self) -> bool {
        self.category() == ProviderCategory::Cloud
    }
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

pub(crate) fn default_sensevoice_language() -> String {
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

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdaterState {
    #[serde(default)]
    pub deferred_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_category_classification() {
        assert_eq!(TranscriptionProvider::Openai.category(), ProviderCategory::Cloud);
        assert_eq!(TranscriptionProvider::Volcengine.category(), ProviderCategory::Cloud);
        assert_eq!(TranscriptionProvider::AliyunAsr.category(), ProviderCategory::Cloud);
        assert_eq!(TranscriptionProvider::AliyunParaformer.category(), ProviderCategory::Cloud);
        assert_eq!(TranscriptionProvider::Sensevoice.category(), ProviderCategory::Local);
    }

    #[test]
    fn provider_is_local_and_is_cloud() {
        assert!(TranscriptionProvider::Openai.is_cloud());
        assert!(!TranscriptionProvider::Openai.is_local());
        assert!(TranscriptionProvider::Sensevoice.is_local());
        assert!(!TranscriptionProvider::Sensevoice.is_cloud());
    }

    #[test]
    fn provider_category_does_not_serialize_into_provider() {
        let provider = TranscriptionProvider::Sensevoice;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"sensevoice\"");

        let provider = TranscriptionProvider::Openai;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"openai\"");
    }
}