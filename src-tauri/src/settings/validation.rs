use crate::sensevoice::model::{spec_for_local_model, LOCAL_MODELS};

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
        settings.text_processing.openai.thinking = legacy_text.thinking;
    }
}

pub(crate) fn normalize_sensevoice_settings(sensevoice: &mut SenseVoiceSettings) {
    let spec = spec_for_local_model(&sensevoice.local_model);
    sensevoice.local_model = spec.model_key.to_string();
    sensevoice.model_id = spec.model_id.to_string();
    sensevoice.language = normalize_sensevoice_language(&sensevoice.language).to_string();
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
    if normalized.privacy.enabled {
        normalized.provider = TranscriptionProvider::Sensevoice;
        normalized.startup.auto_check_updates = false;
    }
    normalized.recording.input_device_name =
        normalized.recording.input_device_name.trim().to_string();
    if normalized.recording.segment_seconds == 0 {
        normalized.recording.segment_seconds = 60;
    }
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
    let Some(spec) = LOCAL_MODELS
        .iter()
        .find(|spec| spec.model_key == sensevoice.local_model)
    else {
        return Err(SettingsError::Serde(format!(
            "SenseVoice local model must be one of a known sherpa-onnx model, got: {}",
            sensevoice.local_model
        )));
    };
    if sensevoice.model_id != spec.model_id {
        return Err(SettingsError::Serde(format!(
            "model ID for {} must use the default value",
            spec.display_name
        )));
    }
    if !matches!(
        sensevoice.language.as_str(),
        "auto" | "zh" | "en" | "ja" | "ko" | "yue"
    ) {
        return Err(SettingsError::Serde(
            "SenseVoice language only supports auto/zh/en/ja/ko/yue".to_string(),
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
    fn normalize_recording_settings_trims_device_and_restores_segment_default() {
        let mut settings = Settings::default();
        settings.recording.segment_seconds = 0;
        settings.recording.input_device_name = "  USB Audio Interface  ".to_string();

        let normalized = normalize_settings(&settings);

        assert_eq!(normalized.recording.segment_seconds, 60);
        assert_eq!(
            normalized.recording.input_device_name,
            "USB Audio Interface"
        );
    }

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
                thinking: ThinkingMode::Off,
            }),
        };

        normalize_text_processing_settings(&mut settings);

        assert_eq!(
            settings.text_processing.provider,
            TextProcessingProvider::Openai
        );
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
        assert_eq!(settings.text_processing.openai.thinking, ThinkingMode::Off);
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

        assert_eq!(
            settings.text_processing.provider,
            TextProcessingProvider::Openai
        );
        assert_eq!(
            settings.text_processing.openai.api_base,
            "https://api.proxy/v1"
        );
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
        assert_eq!(
            restored.text_processing.openai.api_base,
            "https://custom.api/v1"
        );
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
