//! 转写引擎统一接口
//!
//! 将所有转写引擎（OpenAI, 火山引擎, 阿里云 ASR/Paraformer, SenseVoice）
//! 抽象为统一的 `TranscriptionEngine` trait，按云端/本地(Docker/Native)分类。

use crate::aliyun_realtime::{self, AliyunRealtimeError};
use crate::openai::{self, OpenAiError};
use crate::sensevoice::{self, SenseVoiceError};
use crate::settings::{Settings, TranscriptionAlignment, TranscriptionProvider};
use crate::volcengine::{self, VolcengineError};
use std::path::Path;
use thiserror::Error;

// ── 统一结果类型 ──────────────────────────────────────────────

/// 所有转写引擎的统一返回结果
pub struct TranscriptionResult {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}

// ── 统一错误类型 ──────────────────────────────────────────────

/// 转写引擎统一错误，保留原始错误源
#[derive(Debug, Error)]
pub enum TranscriptionError {
    #[error("OpenAI 转写失败: {0}")]
    OpenAi(#[from] OpenAiError),

    #[error("火山引擎转写失败: {0}")]
    Volcengine(#[from] VolcengineError),

    #[error("阿里云转写失败: {0}")]
    Aliyun(#[from] AliyunRealtimeError),

    #[error("SenseVoice 转写失败: {0}")]
    SenseVoice(#[from] SenseVoiceError),
}

// ── 引擎环境分类 ──────────────────────────────────────────────

/// 引擎运行环境分类
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEnvironment {
    /// 云端 API（OpenAI, 火山引擎, 阿里云）
    Cloud,
    /// 本地运行
    Local { runtime: LocalRuntime },
}

/// 本地运行时类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalRuntime {
    /// Docker 容器（SenseVoice Docker, Voxtral, Qwen3-ASR）
    Docker,
    /// 原生二进制（Sherpa-ONNX）
    Native,
}

// ── 引擎 trait ────────────────────────────────────────────────

/// 转写引擎统一接口
pub trait TranscriptionEngine {
    /// 转写音频文件
    fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, TranscriptionError>;

    /// 返回引擎的模型组名称（用于历史记录展示）
    fn model_group(&self) -> String;

    /// 返回引擎运行环境分类
    fn environment(&self) -> EngineEnvironment;
}

// ── OpenAI 引擎 ───────────────────────────────────────────────

pub struct OpenAiEngine {
    settings: Settings,
}

impl TranscriptionEngine for OpenAiEngine {
    fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, TranscriptionError> {
        let text = openai::transcribe_audio(&self.settings, audio_path)?;
        Ok(TranscriptionResult {
            text,
            alignment: None,
        })
    }

    fn model_group(&self) -> String {
        let model = self.settings.openai.speech_to_text.model.trim();
        let display = if model.is_empty() { "-" } else { model };
        format!("OpenAI / {display}")
    }

    fn environment(&self) -> EngineEnvironment {
        EngineEnvironment::Cloud
    }
}

// ── 火山引擎 ──────────────────────────────────────────────────

const VOLCENGINE_FILE_CLUSTER: &str = "volcengine_input_common";
const VOLCENGINE_STREAMING_CLUSTER: &str = "volcengine_streaming_common";

pub struct VolcengineEngine {
    settings: Settings,
}

impl TranscriptionEngine for VolcengineEngine {
    fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, TranscriptionError> {
        let text = volcengine::transcribe_audio(&self.settings, audio_path)?;
        Ok(TranscriptionResult {
            text,
            alignment: None,
        })
    }

    fn model_group(&self) -> String {
        if self.settings.volcengine.use_streaming {
            format!("Volcengine / {VOLCENGINE_STREAMING_CLUSTER}")
        } else if self.settings.volcengine.use_fast {
            format!("Volcengine / {VOLCENGINE_FILE_CLUSTER} (fast)")
        } else {
            format!("Volcengine / {VOLCENGINE_FILE_CLUSTER}")
        }
    }

    fn environment(&self) -> EngineEnvironment {
        EngineEnvironment::Cloud
    }
}

// ── 阿里云 ASR 引擎 ──────────────────────────────────────────

const ALIYUN_ASR_MODEL: &str = "fun-asr-realtime";

pub struct AliyunAsrEngine {
    settings: Settings,
}

impl TranscriptionEngine for AliyunAsrEngine {
    fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, TranscriptionError> {
        let text = aliyun_realtime::transcribe_asr(&self.settings, audio_path)?;
        Ok(TranscriptionResult {
            text,
            alignment: None,
        })
    }

    fn model_group(&self) -> String {
        format!("Aliyun ASR / {ALIYUN_ASR_MODEL}")
    }

    fn environment(&self) -> EngineEnvironment {
        EngineEnvironment::Cloud
    }
}

// ── 阿里云 Paraformer 引擎 ───────────────────────────────────

const ALIYUN_PARAFORMER_MODEL: &str = "paraformer-realtime-v2";

pub struct AliyunParaformerEngine {
    settings: Settings,
}

impl TranscriptionEngine for AliyunParaformerEngine {
    fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, TranscriptionError> {
        let text = aliyun_realtime::transcribe_paraformer(&self.settings, audio_path)?;
        Ok(TranscriptionResult {
            text,
            alignment: None,
        })
    }

    fn model_group(&self) -> String {
        format!("Aliyun Paraformer / {ALIYUN_PARAFORMER_MODEL}")
    }

    fn environment(&self) -> EngineEnvironment {
        EngineEnvironment::Cloud
    }
}

// ── SenseVoice 引擎（本地）────────────────────────────────────

use crate::sensevoice::model::{normalize_local_model, spec_for_local_model, LocalRuntimeKind};

pub struct SenseVoiceEngine {
    settings: Settings,
}

impl SenseVoiceEngine {
    fn display_name(&self) -> &'static str {
        let local_model = normalize_local_model(&self.settings.sensevoice.local_model);
        match local_model {
            "sherpa-onnx-sensevoice" => "Sherpa-ONNX SenseVoice",
            "voxtral" => "Voxtral",
            "qwen3-asr" => "Qwen3-ASR",
            _ => "SenseVoice",
        }
    }
}

impl TranscriptionEngine for SenseVoiceEngine {
    fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult, TranscriptionError> {
        let result = sensevoice::client::transcribe_audio(&self.settings, audio_path)?;
        Ok(TranscriptionResult {
            text: result.text,
            alignment: result.alignment,
        })
    }

    fn model_group(&self) -> String {
        let model_id = self.settings.sensevoice.model_id.trim();
        let display_id = if model_id.is_empty() { "-" } else { model_id };
        format!("{} / {display_id}", self.display_name())
    }

    fn environment(&self) -> EngineEnvironment {
        let local_model = normalize_local_model(&self.settings.sensevoice.local_model);
        let spec = spec_for_local_model(local_model);
        match spec.runtime_kind {
            LocalRuntimeKind::Native => EngineEnvironment::Local {
                runtime: LocalRuntime::Native,
            },
            LocalRuntimeKind::Docker => EngineEnvironment::Local {
                runtime: LocalRuntime::Docker,
            },
        }
    }
}

// ── 引擎工厂 ──────────────────────────────────────────────────

/// 根据设置创建对应的转写引擎
pub fn create_engine(settings: &Settings) -> Box<dyn TranscriptionEngine> {
    match settings.provider {
        TranscriptionProvider::Openai => Box::new(OpenAiEngine {
            settings: settings.clone(),
        }),
        TranscriptionProvider::Volcengine => Box::new(VolcengineEngine {
            settings: settings.clone(),
        }),
        TranscriptionProvider::AliyunAsr => Box::new(AliyunAsrEngine {
            settings: settings.clone(),
        }),
        TranscriptionProvider::AliyunParaformer => Box::new(AliyunParaformerEngine {
            settings: settings.clone(),
        }),
        TranscriptionProvider::Sensevoice => Box::new(SenseVoiceEngine {
            settings: settings.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn create_engine_returns_correct_environment() {
        let mut settings = Settings::default();

        settings.provider = TranscriptionProvider::Openai;
        let engine = create_engine(&settings);
        assert_eq!(engine.environment(), EngineEnvironment::Cloud);

        settings.provider = TranscriptionProvider::Volcengine;
        let engine = create_engine(&settings);
        assert_eq!(engine.environment(), EngineEnvironment::Cloud);

        settings.provider = TranscriptionProvider::AliyunAsr;
        let engine = create_engine(&settings);
        assert_eq!(engine.environment(), EngineEnvironment::Cloud);

        settings.provider = TranscriptionProvider::AliyunParaformer;
        let engine = create_engine(&settings);
        assert_eq!(engine.environment(), EngineEnvironment::Cloud);
    }

    #[test]
    fn create_engine_model_group_matches_legacy() {
        let mut settings = Settings::default();

        settings.provider = TranscriptionProvider::Openai;
        settings.openai.speech_to_text.model = "gpt-4o-transcribe".to_string();
        let engine = create_engine(&settings);
        assert_eq!(engine.model_group(), "OpenAI / gpt-4o-transcribe");

        settings.provider = TranscriptionProvider::Volcengine;
        settings.volcengine.use_streaming = true;
        settings.volcengine.use_fast = false;
        let engine = create_engine(&settings);
        assert_eq!(
            engine.model_group(),
            "Volcengine / volcengine_streaming_common"
        );

        settings.provider = TranscriptionProvider::Volcengine;
        settings.volcengine.use_streaming = false;
        settings.volcengine.use_fast = false;
        let engine = create_engine(&settings);
        assert_eq!(
            engine.model_group(),
            "Volcengine / volcengine_input_common"
        );

        settings.provider = TranscriptionProvider::AliyunAsr;
        let engine = create_engine(&settings);
        assert_eq!(engine.model_group(), "Aliyun ASR / fun-asr-realtime");

        settings.provider = TranscriptionProvider::AliyunParaformer;
        let engine = create_engine(&settings);
        assert_eq!(
            engine.model_group(),
            "Aliyun Paraformer / paraformer-realtime-v2"
        );

        settings.provider = TranscriptionProvider::Sensevoice;
        settings.sensevoice.local_model = "voxtral".to_string();
        settings.sensevoice.model_id =
            "mistralai/Voxtral-Mini-4B-Realtime-2602".to_string();
        let engine = create_engine(&settings);
        assert_eq!(
            engine.model_group(),
            "Voxtral / mistralai/Voxtral-Mini-4B-Realtime-2602"
        );
    }
}
