use std::time::Duration;

pub const LOCAL_MODEL_SENSEVOICE: &str = "sensevoice";
pub const LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE: &str = "sherpa-onnx-sensevoice";
pub const LOCAL_MODEL_VOXTRAL: &str = "voxtral";
pub const LOCAL_MODEL_QWEN3_ASR: &str = "qwen3-asr";

#[allow(dead_code)]
pub const DEFAULT_SENSEVOICE_MODEL_ID: &str = "FunAudioLLM/SenseVoiceSmall";
pub const DEFAULT_VOXTRAL_MODEL_ID: &str = "mistralai/Voxtral-Mini-4B-Realtime-2602";
pub const DEFAULT_QWEN3_ASR_MODEL_ID: &str = "Qwen/Qwen3-ASR-1.7B";
#[allow(dead_code)]
pub const DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID: &str =
    "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
pub const QWEN3_ASR_ALLOWED_MODEL_IDS: [&str; 3] = [
    "Qwen/Qwen3-ASR-1.7B",
    "Qwen/Qwen3-ASR-0.6B",
    "Qwen/Qwen3-ForcedAligner-0.6B",
];

const SENSEVOICE_IMAGE_TAG: &str = "vtt-sensevoice:local";
const VLLM_IMAGE_TAG: &str = "vllm/vllm-openai:nightly";
const SERVICE_CONTAINER_NAME: &str = "vtt-sensevoice-service";
const VLLM_CONTAINER_NAME: &str = "vtt-vllm-service";
const SERVICE_START_TIMEOUT_SECS: u64 = 90;
const VLLM_SERVICE_START_TIMEOUT_SECS: u64 = 5 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalRuntimeKind {
    Native,
    Docker,
}

impl LocalRuntimeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Docker => "docker",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct LocalModelSpec {
    pub model_key: &'static str,
    pub runtime_kind: LocalRuntimeKind,
    pub display_name: &'static str,
    pub supports_pause: bool,
    pub requires_docker: bool,
    pub supports_language: bool,
    pub supports_service_url: bool,
    pub supports_device: bool,
    pub is_vllm: bool,
    pub runtime_image_tag: Option<&'static str>,
    pub container_name: Option<&'static str>,
    pub startup_timeout_secs: u64,
}

pub fn spec_for_local_model(value: &str) -> LocalModelSpec {
    if value.eq_ignore_ascii_case(LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE) {
        return LocalModelSpec {
            model_key: LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE,
            runtime_kind: LocalRuntimeKind::Native,
            display_name: "Sherpa-ONNX SenseVoice",
            supports_pause: false,
            requires_docker: false,
            supports_language: true,
            supports_service_url: false,
            supports_device: true,
            is_vllm: false,
            runtime_image_tag: None,
            container_name: None,
            startup_timeout_secs: 0,
        };
    }
    if value.eq_ignore_ascii_case(LOCAL_MODEL_VOXTRAL) {
        return LocalModelSpec {
            model_key: LOCAL_MODEL_VOXTRAL,
            runtime_kind: LocalRuntimeKind::Docker,
            display_name: "Voxtral",
            supports_pause: true,
            requires_docker: true,
            supports_language: false,
            supports_service_url: true,
            supports_device: true,
            is_vllm: true,
            runtime_image_tag: Some(VLLM_IMAGE_TAG),
            container_name: Some(VLLM_CONTAINER_NAME),
            startup_timeout_secs: VLLM_SERVICE_START_TIMEOUT_SECS,
        };
    }
    if value.eq_ignore_ascii_case(LOCAL_MODEL_QWEN3_ASR) {
        return LocalModelSpec {
            model_key: LOCAL_MODEL_QWEN3_ASR,
            runtime_kind: LocalRuntimeKind::Docker,
            display_name: "Qwen3-ASR",
            supports_pause: true,
            requires_docker: true,
            supports_language: false,
            supports_service_url: true,
            supports_device: true,
            is_vllm: true,
            runtime_image_tag: Some(VLLM_IMAGE_TAG),
            container_name: Some(VLLM_CONTAINER_NAME),
            startup_timeout_secs: VLLM_SERVICE_START_TIMEOUT_SECS,
        };
    }
    LocalModelSpec {
        model_key: LOCAL_MODEL_SENSEVOICE,
        runtime_kind: LocalRuntimeKind::Docker,
        display_name: "SenseVoice",
        supports_pause: true,
        requires_docker: true,
        supports_language: false,
        supports_service_url: true,
        supports_device: true,
        is_vllm: false,
        runtime_image_tag: Some(SENSEVOICE_IMAGE_TAG),
        container_name: Some(SERVICE_CONTAINER_NAME),
        startup_timeout_secs: SERVICE_START_TIMEOUT_SECS,
    }
}

pub fn normalize_local_model(value: &str) -> &'static str {
    spec_for_local_model(value).model_key
}

pub fn supports_sherpa_onnx_target() -> bool {
    !cfg!(all(target_os = "windows", target_arch = "aarch64"))
}

pub fn is_vllm_local_model(value: &str) -> bool {
    spec_for_local_model(value).is_vllm
}

pub fn runtime_image_tag(value: &str) -> &'static str {
    spec_for_local_model(value)
        .runtime_image_tag
        .unwrap_or(SENSEVOICE_IMAGE_TAG)
}

pub fn runtime_container_name(value: &str) -> &'static str {
    spec_for_local_model(value)
        .container_name
        .unwrap_or(SERVICE_CONTAINER_NAME)
}

pub fn service_start_timeout(value: &str) -> Duration {
    Duration::from_secs(spec_for_local_model(value).startup_timeout_secs)
}

pub fn resolve_vllm_model_id(local_model: &str, model_id: &str) -> String {
    match normalize_local_model(local_model) {
        LOCAL_MODEL_VOXTRAL => DEFAULT_VOXTRAL_MODEL_ID.to_string(),
        LOCAL_MODEL_QWEN3_ASR => normalize_qwen3_asr_model_id(model_id).to_string(),
        _ => DEFAULT_QWEN3_ASR_MODEL_ID.to_string(),
    }
}

pub fn normalize_qwen3_asr_model_id(model_id: &str) -> &str {
    let trimmed = model_id.trim();
    QWEN3_ASR_ALLOWED_MODEL_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == trimmed)
        .unwrap_or(DEFAULT_QWEN3_ASR_MODEL_ID)
}
