/// sherpa-onnx 本地模型描述表。
///
/// 本地转写仅通过 sherpa-onnx Rust crate 加载官方预打包 ONNX 模型
/// （GitHub Releases / ModelScope 单包分发，无需 Docker）。
pub const LOCAL_MODEL_SENSEVOICE: &str = "sherpa-onnx-sensevoice";
pub const LOCAL_MODEL_PARAFORMER: &str = "sherpa-onnx-paraformer";
pub const LOCAL_MODEL_FIRERED: &str = "sherpa-onnx-firered";
pub const LOCAL_MODEL_FUNASR_NANO: &str = "sherpa-onnx-funasr-nano";

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
    FunAsrNano,
}

#[derive(Clone, Copy, Debug)]
pub struct LocalModelSpec {
    /// 持久化在设置里的模型 key
    pub model_key: &'static str,
    pub kind: LocalModelKind,
    pub display_name: &'static str,
    /// 模型包解压目录名，同时作为 settings.model_id 的值
    pub model_id: &'static str,
    /// 官方发布的 .tar.bz2 包名
    pub archive_name: &'static str,
    /// 是否在 csukuangfj/asr-models（ModelScope）有镜像
    pub has_modelscope_mirror: bool,
    /// 模型就绪判定所需的文件（相对模型目录）
    pub required_files: &'static [&'static str],
    /// 支持的语言选项（空 = 模型不接受语言参数）
    pub supported_languages: &'static [&'static str],
}

const SPEC_SENSEVOICE: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_SENSEVOICE,
    kind: LocalModelKind::SenseVoice,
    display_name: "SenseVoice-Small",
    model_id: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
    archive_name: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2",
    has_modelscope_mirror: false,
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: &["auto", "zh", "en", "ja", "ko", "yue"],
};

const SPEC_PARAFORMER: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_PARAFORMER,
    kind: LocalModelKind::Paraformer,
    display_name: "Paraformer-zh-small",
    model_id: "sherpa-onnx-paraformer-zh-small-2024-03-09",
    archive_name: "sherpa-onnx-paraformer-zh-small-2024-03-09.tar.bz2",
    has_modelscope_mirror: false,
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_FIRERED: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_FIRERED,
    kind: LocalModelKind::FireRedAsrCtc,
    display_name: "FireRedASR2-CTC",
    model_id: "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25",
    archive_name: "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25.tar.bz2",
    has_modelscope_mirror: true,
    required_files: &["model.int8.onnx", "tokens.txt"],
    supported_languages: &[],
};

const SPEC_FUNASR_NANO: LocalModelSpec = LocalModelSpec {
    model_key: LOCAL_MODEL_FUNASR_NANO,
    kind: LocalModelKind::FunAsrNano,
    display_name: "Fun-ASR-Nano",
    model_id: "sherpa-onnx-funasr-nano-int8-2025-12-30",
    archive_name: "sherpa-onnx-funasr-nano-int8-2025-12-30.tar.bz2",
    has_modelscope_mirror: true,
    required_files: &[
        "encoder_adaptor.int8.onnx",
        "llm.int8.onnx",
        "embedding.int8.onnx",
        "Qwen3-0.6B/tokenizer.json",
        "Qwen3-0.6B/vocab.json",
        "Qwen3-0.6B/merges.txt",
    ],
    supported_languages: &["auto", "zh", "en", "ja"],
};

/// 全部可选的本地模型（顺序即 UI 下拉顺序）
pub const LOCAL_MODELS: [LocalModelSpec; 4] = [
    SPEC_SENSEVOICE,
    SPEC_PARAFORMER,
    SPEC_FIRERED,
    SPEC_FUNASR_NANO,
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
    let mut urls = vec![format!(
        "{}/{}",
        GITHUB_ASR_MODELS_BASE, spec.archive_name
    )];
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
    fn spec_lookup_is_case_insensitive() {
        assert_eq!(
            spec_for_local_model("Sherpa-ONNX-Paraformer").model_key,
            LOCAL_MODEL_PARAFORMER
        );
    }

    #[test]
    fn unknown_and_legacy_docker_models_fall_back_to_default() {
        for legacy in LEGACY_DOCKER_MODEL_KEYS {
            assert_eq!(spec_for_local_model(legacy).model_key, LOCAL_MODEL_SENSEVOICE);
        }
        assert_eq!(spec_for_local_model("no-such-model").model_key, LOCAL_MODEL_SENSEVOICE);
        assert_eq!(spec_for_local_model("").model_key, LOCAL_MODEL_SENSEVOICE);
    }

    #[test]
    fn normalize_local_model_returns_canonical_key() {
        assert_eq!(normalize_local_model("sensevoice"), LOCAL_MODEL_SENSEVOICE);
        assert_eq!(normalize_local_model("sherpa-onnx-firered"), LOCAL_MODEL_FIRERED);
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
