/// sherpa-onnx 本地模型描述表。
///
/// 本地转写仅通过 sherpa-onnx Rust crate 加载官方预打包 ONNX 模型
/// （GitHub Releases / ModelScope 单包分发）。
/// 模型系列 key；同时是各系列默认规模的持久化 key（兼容旧配置）。
pub const LOCAL_MODEL_SENSEVOICE: &str = "sherpa-onnx-sensevoice";
pub const LOCAL_MODEL_PARAFORMER: &str = "sherpa-onnx-paraformer";
pub const LOCAL_MODEL_FIRERED: &str = "sherpa-onnx-firered";
pub const LOCAL_MODEL_FUNASR_NANO: &str = "sherpa-onnx-funasr-nano";
pub const LOCAL_MODEL_PARAKEET: &str = "sherpa-onnx-parakeet";
pub const LOCAL_MODEL_MOONSHINE: &str = "sherpa-onnx-moonshine";
pub const LOCAL_MODEL_NEMOTRON: &str = "sherpa-onnx-nemotron";

pub const LOCAL_MODEL_SENSEVOICE_FP32: &str = "sherpa-onnx-sensevoice-fp32";
pub const LOCAL_MODEL_PARAFORMER_LARGE_INT8: &str = "sherpa-onnx-paraformer-large-int8";
pub const LOCAL_MODEL_PARAFORMER_LARGE: &str = "sherpa-onnx-paraformer-large";
pub const LOCAL_MODEL_FIRERED_LARGE: &str = "sherpa-onnx-firered-large";
pub const LOCAL_MODEL_FUNASR_NANO_FP32: &str = "sherpa-onnx-funasr-nano-fp32";
pub const LOCAL_MODEL_MOONSHINE_BASE: &str = "sherpa-onnx-moonshine-base";
pub const LOCAL_MODEL_NEMOTRON_MULTILINGUAL: &str = "sherpa-onnx-nemotron-multilingual";

pub const DEFAULT_LOCAL_MODEL: &str = LOCAL_MODEL_SENSEVOICE;

const GITHUB_ASR_MODELS_BASE: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models";
const MODELSCOPE_ASR_MODELS_BASE: &str =
    "https://modelscope.cn/models/csukuangfj/asr-models/resolve/master";

/// Moonshine v2 在长音频上会输出大量重复词，转写前需切成不超过该长度的段
const MOONSHINE_MAX_CHUNK_SECONDS: u64 = 20;

/// sherpa-onnx 识别器使用的模型类别（决定加载哪种识别器及其配置字段）。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalModelKind {
    SenseVoice,
    Paraformer,
    FireRedAsrCtc,
    /// FireRedASR AED（encoder + decoder）
    FireRedAsr,
    FunAsrNano,
    /// NeMo TDT 离线 transducer（Parakeet，encoder + decoder + joiner）
    NemoTransducer,
    /// Moonshine v2（encoder + merged decoder）
    MoonshineV2,
    /// 流式 transducer（Nemotron，encoder + decoder + joiner），使用 OnlineRecognizer
    OnlineTransducer,
}

impl LocalModelKind {
    /// 是否为流式模型（录音时边说边识别）
    pub fn is_streaming(self) -> bool {
        matches!(self, LocalModelKind::OnlineTransducer)
    }

    /// 单段音频的最长秒数；None 表示沿用用户设置的分段秒数
    pub fn max_chunk_seconds(self) -> Option<u64> {
        match self {
            LocalModelKind::MoonshineV2 => Some(MOONSHINE_MAX_CHUNK_SECONDS),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalModelSpec {
    /// 持久化在设置里的模型 key（每个规模一个）
    pub model_key: &'static str,
    /// 所属模型系列（系列默认规模的 model_key）
    pub family: &'static str,
    pub kind: LocalModelKind,
    pub display_name: &'static str,
    /// 模型包解压目录名，同时作为 settings.model_id 的值
    pub model_id: &'static str,
    /// 官方发布的 .tar.bz2 包名
    pub archive_name: &'static str,
    /// 是否在 csukuangfj/asr-models（ModelScope）有镜像
    pub has_modelscope_mirror: bool,
    /// 识别器加载的 ONNX 文件，按 kind 约定顺序：
    /// 单文件模型 [model]；FireRedAsr / MoonshineV2 [encoder, decoder]；
    /// FunAsrNano [encoder_adaptor, llm, embedding]；
    /// NemoTransducer / OnlineTransducer [encoder, decoder, joiner]
    pub model_files: &'static [&'static str],
    /// 模型就绪判定所需的文件（相对模型目录）
    pub required_files: &'static [&'static str],
    /// 支持的语言选项（空 = 模型不接受语言参数）
    pub supported_languages: &'static [&'static str],
}

const SENSEVOICE_LANGUAGES: &[&str] = &["auto", "zh", "en", "ja", "ko", "yue"];
const FUNASR_NANO_LANGUAGES: &[&str] = &["auto", "zh", "en", "ja"];

const SPEC_SENSEVOICE: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_SENSEVOICE,
    family: LOCAL_MODEL_SENSEVOICE,
    kind: LocalModelKind::SenseVoice,
    display_name: "SenseVoice-Small",
    model_id: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
    archive_name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2",
    has_modelscope_mirror: false,
    model_files: &["model.int8.onnx"],
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: SENSEVOICE_LANGUAGES,
};

const SPEC_SENSEVOICE_FP32: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_SENSEVOICE_FP32,
    family: LOCAL_MODEL_SENSEVOICE,
    kind: LocalModelKind::SenseVoice,
    display_name: "SenseVoice-Small-FP32",
    model_id: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
    archive_name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2",
    has_modelscope_mirror: false,
    model_files: &["model.onnx"],
    required_files: &["model.onnx", "tokens.txt"],
    supported_languages: SENSEVOICE_LANGUAGES,
};

const SPEC_PARAFORMER: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_PARAFORMER,
    family: LOCAL_MODEL_PARAFORMER,
    kind: LocalModelKind::Paraformer,
    display_name: "Paraformer-zh-small",
    model_id: "sherpa-onnx-paraformer-zh-small-2024-03-09",
    archive_name: "sherpa-onnx-paraformer-zh-small-2024-03-09.tar.bz2",
    has_modelscope_mirror: false,
    model_files: &["model.int8.onnx"],
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_PARAFORMER_LARGE_INT8: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_PARAFORMER_LARGE_INT8,
    family: LOCAL_MODEL_PARAFORMER,
    kind: LocalModelKind::Paraformer,
    display_name: "Paraformer-zh-large-int8",
    model_id: "sherpa-onnx-paraformer-zh-int8-2025-10-07",
    archive_name: "sherpa-onnx-paraformer-zh-int8-2025-10-07.tar.bz2",
    has_modelscope_mirror: false,
    model_files: &["model.int8.onnx"],
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_PARAFORMER_LARGE: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_PARAFORMER_LARGE,
    family: LOCAL_MODEL_PARAFORMER,
    kind: LocalModelKind::Paraformer,
    display_name: "Paraformer-zh-large",
    model_id: "sherpa-onnx-paraformer-zh-2025-10-07",
    archive_name: "sherpa-onnx-paraformer-zh-2025-10-07.tar.bz2",
    has_modelscope_mirror: false,
    model_files: &["model.onnx"],
    required_files: &["model.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_FIRERED: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_FIRERED,
    family: LOCAL_MODEL_FIRERED,
    kind: LocalModelKind::FireRedAsrCtc,
    display_name: "FireRedASR2-CTC",
    model_id: "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25",
    archive_name: "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25.tar.bz2",
    has_modelscope_mirror: true,
    model_files: &["model.int8.onnx"],
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_FIRERED_LARGE: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_FIRERED_LARGE,
    family: LOCAL_MODEL_FIRERED,
    kind: LocalModelKind::FireRedAsr,
    display_name: "FireRedASR-Large",
    model_id: "sherpa-onnx-fire-red-asr-large-zh_en-2025-02-16",
    archive_name: "sherpa-onnx-fire-red-asr-large-zh_en-2025-02-16.tar.bz2",
    has_modelscope_mirror: false,
    model_files: &["encoder.int8.onnx", "decoder.int8.onnx"],
    required_files: &["encoder.int8.onnx", "decoder.int8.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_FUNASR_NANO: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_FUNASR_NANO,
    family: LOCAL_MODEL_FUNASR_NANO,
    kind: LocalModelKind::FunAsrNano,
    display_name: "Fun-ASR-Nano",
    model_id: "sherpa-onnx-funasr-nano-int8-2025-12-30",
    archive_name: "sherpa-onnx-funasr-nano-int8-2025-12-30.tar.bz2",
    has_modelscope_mirror: true,
    model_files: &[
        "encoder_adaptor.int8.onnx",
        "llm.int8.onnx",
        "embedding.int8.onnx",
    ],
    required_files: &[
        "encoder_adaptor.int8.onnx",
        "llm.int8.onnx",
        "embedding.int8.onnx",
        "Qwen3-0.6B/tokenizer.json",
        "Qwen3-0.6B/vocab.json",
        "Qwen3-0.6B/merges.txt",
    ],
    supported_languages: FUNASR_NANO_LANGUAGES,
};

const SPEC_FUNASR_NANO_FP32: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_FUNASR_NANO_FP32,
    family: LOCAL_MODEL_FUNASR_NANO,
    kind: LocalModelKind::FunAsrNano,
    display_name: "Fun-ASR-Nano-FP32",
    model_id: "sherpa-onnx-funasr-nano-2025-12-30",
    archive_name: "sherpa-onnx-funasr-nano-2025-12-30.tar.bz2",
    has_modelscope_mirror: true,
    model_files: &["encoder_adaptor.onnx", "llm.fp32.onnx", "embedding.onnx"],
    required_files: &[
        "encoder_adaptor.onnx",
        "llm.fp32.onnx",
        "llm.fp32.data",
        "embedding.onnx",
        "Qwen3-0.6B/tokenizer.json",
        "Qwen3-0.6B/vocab.json",
        "Qwen3-0.6B/merges.txt",
    ],
    supported_languages: FUNASR_NANO_LANGUAGES,
};

const TRANSDUCER_INT8_FILES: &[&str] =
    &["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx"];
const TRANSDUCER_INT8_REQUIRED: &[&str] = &[
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];
const MOONSHINE_V2_FILES: &[&str] = &["encoder_model.ort", "decoder_model_merged.ort"];
const MOONSHINE_V2_REQUIRED: &[&str] = &[
    "encoder_model.ort",
    "decoder_model_merged.ort",
    "tokens.txt",
];

const SPEC_PARAKEET: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_PARAKEET,
    family: LOCAL_MODEL_PARAKEET,
    kind: LocalModelKind::NemoTransducer,
    display_name: "Parakeet-TDT-0.6B-v3",
    model_id: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
    archive_name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
    has_modelscope_mirror: false,
    model_files: TRANSDUCER_INT8_FILES,
    required_files: TRANSDUCER_INT8_REQUIRED,
    supported_languages: &[],
};

const SPEC_MOONSHINE: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_MOONSHINE,
    family: LOCAL_MODEL_MOONSHINE,
    kind: LocalModelKind::MoonshineV2,
    display_name: "Moonshine-tiny-en",
    model_id: "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27",
    archive_name: "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27.tar.bz2",
    has_modelscope_mirror: true,
    model_files: MOONSHINE_V2_FILES,
    required_files: MOONSHINE_V2_REQUIRED,
    supported_languages: &[],
};

const SPEC_MOONSHINE_BASE: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_MOONSHINE_BASE,
    family: LOCAL_MODEL_MOONSHINE,
    kind: LocalModelKind::MoonshineV2,
    display_name: "Moonshine-base-en",
    model_id: "sherpa-onnx-moonshine-base-en-quantized-2026-02-27",
    archive_name: "sherpa-onnx-moonshine-base-en-quantized-2026-02-27.tar.bz2",
    has_modelscope_mirror: true,
    model_files: MOONSHINE_V2_FILES,
    required_files: MOONSHINE_V2_REQUIRED,
    supported_languages: &[],
};

const SPEC_NEMOTRON: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_NEMOTRON,
    family: LOCAL_MODEL_NEMOTRON,
    kind: LocalModelKind::OnlineTransducer,
    display_name: "Nemotron-Streaming-EN-0.6B",
    model_id: "sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25",
    archive_name: "sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25.tar.bz2",
    has_modelscope_mirror: false,
    model_files: TRANSDUCER_INT8_FILES,
    required_files: TRANSDUCER_INT8_REQUIRED,
    supported_languages: &[],
};

const SPEC_NEMOTRON_MULTILINGUAL: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_NEMOTRON_MULTILINGUAL,
    family: LOCAL_MODEL_NEMOTRON,
    kind: LocalModelKind::OnlineTransducer,
    display_name: "Nemotron-3.5-Streaming-0.6B",
    model_id: "sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11",
    archive_name: "sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11.tar.bz2",
    has_modelscope_mirror: false,
    model_files: TRANSDUCER_INT8_FILES,
    required_files: TRANSDUCER_INT8_REQUIRED,
    supported_languages: &[],
};

/// 全部可选的本地模型（顺序即 UI 下拉顺序；同系列按规模从小到大）
pub const LOCAL_MODELS: [LocalModelSpec; 14] = [
    SPEC_SENSEVOICE,
    SPEC_SENSEVOICE_FP32,
    SPEC_PARAFORMER,
    SPEC_PARAFORMER_LARGE_INT8,
    SPEC_PARAFORMER_LARGE,
    SPEC_FIRERED,
    SPEC_FIRERED_LARGE,
    SPEC_FUNASR_NANO,
    SPEC_FUNASR_NANO_FP32,
    SPEC_PARAKEET,
    SPEC_MOONSHINE,
    SPEC_MOONSHINE_BASE,
    SPEC_NEMOTRON,
    SPEC_NEMOTRON_MULTILINGUAL,
];

/// 解析 local_model 为模型描述符。
/// 未识别的值一律回落到默认 SenseVoice。
pub fn spec_for_local_model(value: &str) -> &'static LocalModelSpec {
    for spec in LOCAL_MODELS.iter() {
        if value.eq_ignore_ascii_case(spec.model_key) {
            return spec;
        }
    }
    &SPEC_SENSEVOICE
}

pub fn normalize_local_model(value: &str) -> &'static str {
    spec_for_local_model(value).model_key
}

/// 模型包的下载地址列表（GitHub 主源 + 可选 ModelScope 镜像回退）
pub fn download_urls(spec: &LocalModelSpec) -> Vec<String> {
    let mut urls = vec![format!("{}/{}", GITHUB_ASR_MODELS_BASE, spec.archive_name)];
    if spec.has_modelscope_mirror {
        urls.push(format!(
            "{}/{}",
            MODELSCOPE_ASR_MODELS_BASE, spec.archive_name
        ));
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_spec_is_well_formed() {
        for spec in LOCAL_MODELS.iter() {
            assert!(!spec.model_key.is_empty());
            assert!(!spec.display_name.is_empty());
            assert!(!spec.model_id.is_empty());
            assert_eq!(spec.archive_name, format!("{}.tar.bz2", spec.model_id));
            assert!(!spec.required_files.is_empty());
            for file in spec.model_files {
                assert!(spec.required_files.contains(file));
            }
            let expected_model_files = match spec.kind {
                LocalModelKind::FireRedAsr | LocalModelKind::MoonshineV2 => 2,
                LocalModelKind::FunAsrNano
                | LocalModelKind::NemoTransducer
                | LocalModelKind::OnlineTransducer => 3,
                _ => 1,
            };
            assert_eq!(spec.model_files.len(), expected_model_files);
            assert!(LOCAL_MODELS
                .iter()
                .any(|family| family.model_key == spec.family));
            // Fun-ASR-Nano 使用 Qwen3 tokenizer 目录，其余模型使用 tokens.txt
            if spec.kind == LocalModelKind::FunAsrNano {
                assert!(spec
                    .required_files
                    .iter()
                    .any(|f| f.starts_with("Qwen3-0.6B/")));
            } else {
                assert!(spec.required_files.iter().any(|f| *f == "tokens.txt"));
            }
        }
    }

    #[test]
    fn model_keys_and_dirs_are_unique() {
        for (i, a) in LOCAL_MODELS.iter().enumerate() {
            for b in LOCAL_MODELS.iter().skip(i + 1) {
                assert_ne!(a.model_key, b.model_key);
                assert_ne!(a.model_id, b.model_id);
            }
        }
    }

    #[test]
    fn spec_lookup_is_case_insensitive() {
        assert_eq!(
            spec_for_local_model("Sherpa-ONNX-Paraformer").model_key,
            LOCAL_MODEL_PARAFORMER
        );
    }

    #[test]
    fn unknown_models_fall_back_to_default() {
        assert_eq!(
            spec_for_local_model("no-such-model").model_key,
            LOCAL_MODEL_SENSEVOICE
        );
        assert_eq!(spec_for_local_model("").model_key, LOCAL_MODEL_SENSEVOICE);
    }

    #[test]
    fn normalize_local_model_returns_canonical_key() {
        assert_eq!(
            normalize_local_model("Sherpa-ONNX-SenseVoice"),
            LOCAL_MODEL_SENSEVOICE
        );
        assert_eq!(
            normalize_local_model("sherpa-onnx-firered"),
            LOCAL_MODEL_FIRERED
        );
    }

    #[test]
    fn download_urls_primary_is_github_and_mirror_only_when_marked() {
        let sensevoice = spec_for_local_model(LOCAL_MODEL_SENSEVOICE);
        let urls = download_urls(sensevoice);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].starts_with(GITHUB_ASR_MODELS_BASE));
        assert!(urls[0].ends_with(sensevoice.archive_name));

        let funasr = spec_for_local_model(LOCAL_MODEL_FUNASR_NANO);
        let urls = download_urls(funasr);
        assert_eq!(urls.len(), 2);
        assert!(urls[1].starts_with(MODELSCOPE_ASR_MODELS_BASE));
        assert!(urls[1].ends_with(funasr.archive_name));
    }

    #[test]
    fn only_nemotron_family_is_streaming() {
        for spec in LOCAL_MODELS.iter() {
            assert_eq!(
                spec.kind.is_streaming(),
                spec.family == LOCAL_MODEL_NEMOTRON,
                "{}",
                spec.model_key
            );
        }
    }

    #[test]
    fn moonshine_limits_chunk_length() {
        assert_eq!(
            spec_for_local_model(LOCAL_MODEL_MOONSHINE_BASE)
                .kind
                .max_chunk_seconds(),
            Some(MOONSHINE_MAX_CHUNK_SECONDS)
        );
        assert_eq!(
            spec_for_local_model(LOCAL_MODEL_PARAKEET)
                .kind
                .max_chunk_seconds(),
            None
        );
    }
}
