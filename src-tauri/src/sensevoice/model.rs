/// sherpa-onnx 本地模型描述表。
///
/// 本地转写仅通过 sherpa-onnx Rust crate 加载官方预打包 ONNX 模型
/// （GitHub Releases / ModelScope 单包分发，无需 Docker）。
/// 模型系列 key；同时是各系列默认规模的持久化 key（兼容旧配置）。
pub const LOCAL_MODEL_SENSEVOICE: &str = "sherpa-onnx-sensevoice";
pub const LOCAL_MODEL_PARAFORMER: &str = "sherpa-onnx-paraformer";
pub const LOCAL_MODEL_FIRERED: &str = "sherpa-onnx-firered";
pub const LOCAL_MODEL_FUNASR_NANO: &str = "sherpa-onnx-funasr-nano";

pub const LOCAL_MODEL_SENSEVOICE_FP32: &str = "sherpa-onnx-sensevoice-fp32";
pub const LOCAL_MODEL_PARAFORMER_LARGE_INT8: &str = "sherpa-onnx-paraformer-large-int8";
pub const LOCAL_MODEL_PARAFORMER_LARGE: &str = "sherpa-onnx-paraformer-large";
pub const LOCAL_MODEL_FIRERED_LARGE: &str = "sherpa-onnx-firered-large";
pub const LOCAL_MODEL_FUNASR_NANO_FP32: &str = "sherpa-onnx-funasr-nano-fp32";

pub const DEFAULT_LOCAL_MODEL: &str = LOCAL_MODEL_SENSEVOICE;

const GITHUB_ASR_MODELS_BASE: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models";
const MODELSCOPE_ASR_MODELS_BASE: &str =
    "https://modelscope.cn/models/csukuangfj/asr-models/resolve/master";

/// sherpa-onnx 离线识别器使用的模型类别（决定 OfflineModelConfig 的哪个字段生效）。
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalModelKind {
    SenseVoice,
    Paraformer,
    FireRedAsrCtc,
    /// FireRedASR AED（encoder + decoder）
    FireRedAsr,
    FunAsrNano,
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
    /// 单文件模型 [model]；FireRedAsr [encoder, decoder]；
    /// FunAsrNano [encoder_adaptor, llm, embedding]
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

/// 全部可选的本地模型（顺序即 UI 下拉顺序；同系列按规模从小到大）
pub const LOCAL_MODELS: [LocalModelSpec; 9] = [
    SPEC_SENSEVOICE,
    SPEC_SENSEVOICE_FP32,
    SPEC_PARAFORMER,
    SPEC_PARAFORMER_LARGE_INT8,
    SPEC_PARAFORMER_LARGE,
    SPEC_FIRERED,
    SPEC_FIRERED_LARGE,
    SPEC_FUNASR_NANO,
    SPEC_FUNASR_NANO_FP32,
];

/// 已移除的 Docker 本地模型 key（用于旧配置迁移识别）
#[cfg(test)]
const LEGACY_DOCKER_MODEL_KEYS: [&str; 3] = ["sensevoice", "voxtral", "qwen3-asr"];

/// 解析 local_model 为模型描述符。
/// 未识别值（包括已移除的 Docker 模型 key）一律回落到默认 SenseVoice。
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
                LocalModelKind::FireRedAsr => 2,
                LocalModelKind::FunAsrNano => 3,
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
    fn unknown_and_legacy_docker_models_fall_back_to_default() {
        for legacy in LEGACY_DOCKER_MODEL_KEYS {
            assert_eq!(
                spec_for_local_model(legacy).model_key,
                LOCAL_MODEL_SENSEVOICE
            );
        }
        assert_eq!(
            spec_for_local_model("no-such-model").model_key,
            LOCAL_MODEL_SENSEVOICE
        );
        assert_eq!(spec_for_local_model("").model_key, LOCAL_MODEL_SENSEVOICE);
    }

    #[test]
    fn normalize_local_model_returns_canonical_key() {
        assert_eq!(normalize_local_model("sensevoice"), LOCAL_MODEL_SENSEVOICE);
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
}
