use crate::sensevoice::model::supports_sherpa_onnx_target;
use url::Url;

use super::storage::SettingsError;
use super::types::*;

pub(crate) fn normalize_text_processing_settings(settings: &mut Settings) {
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

pub(crate) fn normalize_sensevoice_settings(sensevoice: &mut SenseVoiceSettings) {
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

pub(crate) fn normalize_settings(settings: &Settings) -> Settings {
    let mut normalized = settings.clone();
    normalize_sensevoice_settings(&mut normalized.sensevoice);
    normalize_aliyun_settings(&mut normalized.aliyun, &normalized.provider);
    normalize_text_processing_settings(&mut normalized);
    normalized
}

pub(crate) fn validate_settings(settings: &Settings) -> Result<(), SettingsError> {
    let required = ["translate", "polish"];
    for id in required {
        let exists = settings
            .triggers
            .iter()
            .any(|card| card.id == id && card.locked);
        if !exists {
            return Err(SettingsError::Serde(format!("闂婎偄娲ら幊姗€濡磋箛鏃傗攳婵犻潧鐗婂▓宀勬煕閹邦剚鍣介柣銈呮閹叉挳鏁冮埀顒冦亹閸屾粍瀚? {id}")));
        }
    }

    for card in &settings.triggers {
        let has_value = card.variables.iter().any(|value| !value.trim().is_empty());
        if !has_value {
            return Err(SettingsError::Serde(format!(
                "闁荤喐鐟辩粻鎴ｃ亹閸屾粍瀚氱€广儱鎳庡畷鍓佺礄閻樼數鎲归梺鑹版珪閸ㄦ繄鎷归悢铏圭煔? {}",
                card.title
            )));
        }
        if card.keyword.trim().is_empty() {
            return Err(SettingsError::Serde(format!(
                "闁荤喐鐟辩粻鎴ｃ亹閸屾粍瀚氱€广儱鎳庤ぐ鐘绘⒑濞嗘儳鏋涢柣鈯欏嫮鈻旂€广儱鐗嗛崢鏉戔槈閹捐顏犻柍? {}",
                card.title
            )));
        }
        if card.keyword.matches("{value}").count() > 1 {
            return Err(SettingsError::Serde(format!(
                "闁荤喐鐟辩粻鎴ｃ亹閸屾粍瀚氱€广儱鎳庤ぐ鐘绘⒑濞嗘儳鏋涢柣鈯欏洤瀚夐柍褜鍓氬鍕潩椤掆偓閻﹀爼鏌涘鐓庝簵缂佹柨鐡ㄧ粙?{{value}}: {}",
                card.title
            )));
        }
    }

    validate_sensevoice_settings(&settings.sensevoice)?;
    validate_aliyun_settings(settings)?;
    Ok(())
}

pub(crate) fn validate_sensevoice_settings(
    sensevoice: &SenseVoiceSettings,
) -> Result<(), SettingsError> {
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
                "SenseVoice 闂佸搫鐗嗙粔瀛樻叏閻旂厧鎹堕柡澶嬪缁插鏌￠崘顏勎ｉ柡? {err}"
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
                "SenseVoice 闂佸搫鐗嗙粔瀛樻叏閻旂厧鎹堕柡澶嬪缁插鐓崶褎鍤囬柕鍡楃箲閹峰懐鎹勯妸锔芥 http 闂?https".to_string(),
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
            "SenseVoice 闂佽浜介崝搴ㄥ箖婵犲嫭濯奸柟顖嗗本校婵炲濮撮幊蹇涘极椤曗偓楠?auto/cpu/cuda".to_string(),
        ));
    }
    if !matches!(
        sensevoice.stop_mode.as_str(),
        STOP_MODE_STOP | STOP_MODE_PAUSE
    ) {
        return Err(SettingsError::Serde(
            "SenseVoice 闂佺顑嗙划宥夘敆濞戞鐔煎灳瀹曞洨顢呮繛瀵稿Т閹冲繘寮鈧獮?stop/pause"
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

fn validate_aliyun_settings(settings: &Settings) -> Result<(), SettingsError> {
    let region = settings.aliyun.region.as_str();
    if !matches!(region, ALIYUN_REGION_BEIJING | ALIYUN_REGION_SINGAPORE) {
        return Err(SettingsError::Serde(
            "闂傚倸鍟锟犲闯闁垮顩查柟瀛樼箖閸曢箖鏌涢埡鍐ㄦ瀺缂侇喖閰ｅ銊╊敍濞戞妲?beijing/singapore".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn text_processing_settings_round_trip_preserves_api_key() {
        let mut settings = Settings::default();
        settings.text_processing.openai.api_key = "my-text-api-key".to_string();
        settings.text_processing.openai.api_base = "https://custom.api/v1".to_string();
        settings.text_processing.openai.model = "gpt-4o".to_string();
        settings.text_processing.openai.temperature = 0.3;

        let json = serde_json::to_string(&settings).expect("序列化失败");
        let restored: Settings = serde_json::from_str(&json).expect("反序列化失败");

        assert_eq!(restored.text_processing.openai.api_key, "my-text-api-key");
        assert_eq!(restored.text_processing.openai.api_base, "https://custom.api/v1");
        assert_eq!(restored.text_processing.openai.model, "gpt-4o");
        assert!((restored.text_processing.openai.temperature - 0.3_f32).abs() < 1e-6);
    }

    #[test]
    fn normalize_text_processing_does_not_overwrite_existing_api_key() {
        let mut settings = Settings::default();
        settings.openai.api_key = "transcription-key".to_string();
        settings.text_processing.openai.api_key = "text-only-key".to_string();

        normalize_text_processing_settings(&mut settings);

        // Custom text processing key must NOT be overwritten by transcription key
        assert_eq!(settings.text_processing.openai.api_key, "text-only-key");
    }
}
