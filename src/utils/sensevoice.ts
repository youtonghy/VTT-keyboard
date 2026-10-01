export const LOCAL_MODEL_SENSEVOICE = "sherpa-onnx-sensevoice";
export const LOCAL_MODEL_PARAFORMER = "sherpa-onnx-paraformer";
export const LOCAL_MODEL_FIRERED = "sherpa-onnx-firered";
export const LOCAL_MODEL_FUNASR_NANO = "sherpa-onnx-funasr-nano";

export interface LocalModelSpec {
  /** 持久化在设置里的模型 key（与后端一致） */
  key: string;
  /** 展示名（历史记录 modelGroup 同源） */
  displayName: string;
  /** 模型包解压目录名，同时作为 settings.modelId */
  modelId: string;
  /** i18n 下拉选项文案 key */
  labelKey: string;
  /** 支持的语言选项；空数组 = 模型不接受语言参数（UI 隐藏语言下拉） */
  supportedLanguages: string[];
}

export const LOCAL_MODELS: readonly LocalModelSpec[] = [
  {
    key: LOCAL_MODEL_SENSEVOICE,
    displayName: "SenseVoice-Small",
    modelId: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09",
    labelKey: "sensevoice.localModelSenseVoice",
    supportedLanguages: ["auto", "zh", "en", "ja", "ko", "yue"],
  },
  {
    key: LOCAL_MODEL_PARAFORMER,
    displayName: "Paraformer-zh-small",
    modelId: "sherpa-onnx-paraformer-zh-small-2024-03-09",
    labelKey: "sensevoice.localModelParaformer",
    supportedLanguages: [],
  },
  {
    key: LOCAL_MODEL_FIRERED,
    displayName: "FireRedASR2-CTC",
    modelId: "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25",
    labelKey: "sensevoice.localModelFireRed",
    supportedLanguages: [],
  },
  {
    key: LOCAL_MODEL_FUNASR_NANO,
    displayName: "Fun-ASR-Nano",
    modelId: "sherpa-onnx-funasr-nano-int8-2025-12-30",
    labelKey: "sensevoice.localModelFunasrNano",
    supportedLanguages: ["auto", "zh", "en", "ja"],
  },
];

const DEFAULT_MODEL = LOCAL_MODELS[0];

export const LANGUAGE_OPTIONS = [
  { value: "auto", labelKey: "sensevoice.languageAuto" },
  { value: "zh", labelKey: "sensevoice.languageZh" },
  { value: "en", labelKey: "sensevoice.languageEn" },
  { value: "ja", labelKey: "sensevoice.languageJa" },
  { value: "ko", labelKey: "sensevoice.languageKo" },
  { value: "yue", labelKey: "sensevoice.languageYue" },
] as const;

export const specForLocalModel = (value: string | undefined): LocalModelSpec =>
  LOCAL_MODELS.find((spec) => spec.key === value) ?? DEFAULT_MODEL;

export const normalizeLocalModel = (value: string | undefined) =>
  specForLocalModel(value).key;

export const getDefaultModelId = (localModel: string | undefined) =>
  specForLocalModel(localModel).modelId;

/** 当前模型支持的语言选项；空列表表示模型不接受语言参数 */
export const languageOptionsFor = (localModel: string | undefined) =>
  specForLocalModel(localModel).supportedLanguages;

export const normalizeSenseVoiceLanguage = (
  language: string | undefined,
  localModel?: string | undefined
) => {
  const supported = languageOptionsFor(localModel);
  const trimmed = language?.trim();
  if (supported.length > 0 && trimmed && supported.includes(trimmed)) {
    return trimmed;
  }
  return "auto";
};

export const formatBytes = (value: number | undefined) => {
  if (value === undefined || !Number.isFinite(value) || value < 0) {
    return "";
  }
  if (value < 1024) {
    return `${value} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let next = value / 1024;
  let index = 0;
  while (next >= 1024 && index < units.length - 1) {
    next /= 1024;
    index += 1;
  }
  return `${next.toFixed(next >= 100 ? 0 : next >= 10 ? 1 : 2)} ${units[index]}`;
};
