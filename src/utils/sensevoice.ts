/** 模型系列 key；同时是各系列默认规模的持久化 key（与后端一致） */
export const LOCAL_MODEL_SENSEVOICE = "sherpa-onnx-sensevoice";
export const LOCAL_MODEL_PARAFORMER = "sherpa-onnx-paraformer";
export const LOCAL_MODEL_FIRERED = "sherpa-onnx-firered";
export const LOCAL_MODEL_FUNASR_NANO = "sherpa-onnx-funasr-nano";

export interface LocalModelSpec {
  /** 持久化在设置里的模型 key（与后端一致，每个规模一个） */
  key: string;
  /** 所属模型系列（系列默认规模的 key） */
  family: string;
  /** 展示名（历史记录 modelGroup 同源） */
  displayName: string;
  /** 模型包解压目录名，同时作为 settings.modelId */
  modelId: string;
  /** i18n 规模下拉选项文案 key */
  sizeLabelKey: string;
  /** 支持的语言选项；空数组 = 模型不接受语言参数（UI 隐藏语言下拉） */
  supportedLanguages: string[];
}

export interface LocalModelFamily {
  key: string;
  /** i18n 系列下拉选项文案 key */
  labelKey: string;
}

const SENSEVOICE_LANGUAGES = ["auto", "zh", "en", "ja", "ko", "yue"];
const FUNASR_NANO_LANGUAGES = ["auto", "zh", "en", "ja"];

export const LOCAL_MODEL_FAMILIES: readonly LocalModelFamily[] = [
  { key: LOCAL_MODEL_SENSEVOICE, labelKey: "sensevoice.localModelSenseVoice" },
  { key: LOCAL_MODEL_PARAFORMER, labelKey: "sensevoice.localModelParaformer" },
  { key: LOCAL_MODEL_FIRERED, labelKey: "sensevoice.localModelFireRed" },
  { key: LOCAL_MODEL_FUNASR_NANO, labelKey: "sensevoice.localModelFunasrNano" },
];

/** 全部可选的本地模型（同系列按规模从小到大） */
export const LOCAL_MODELS: readonly LocalModelSpec[] = [
  {
    key: LOCAL_MODEL_SENSEVOICE,
    family: LOCAL_MODEL_SENSEVOICE,
    displayName: "SenseVoice-Small",
    modelId: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
    sizeLabelKey: "sensevoice.modelSize.senseVoiceInt8",
    supportedLanguages: SENSEVOICE_LANGUAGES,
  },
  {
    key: "sherpa-onnx-sensevoice-fp32",
    family: LOCAL_MODEL_SENSEVOICE,
    displayName: "SenseVoice-Small-FP32",
    modelId: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
    sizeLabelKey: "sensevoice.modelSize.senseVoiceFp32",
    supportedLanguages: SENSEVOICE_LANGUAGES,
  },
  {
    key: LOCAL_MODEL_PARAFORMER,
    family: LOCAL_MODEL_PARAFORMER,
    displayName: "Paraformer-zh-small",
    modelId: "sherpa-onnx-paraformer-zh-small-2024-03-09",
    sizeLabelKey: "sensevoice.modelSize.paraformerSmall",
    supportedLanguages: [],
  },
  {
    key: "sherpa-onnx-paraformer-large-int8",
    family: LOCAL_MODEL_PARAFORMER,
    displayName: "Paraformer-zh-large-int8",
    modelId: "sherpa-onnx-paraformer-zh-int8-2025-10-07",
    sizeLabelKey: "sensevoice.modelSize.paraformerLargeInt8",
    supportedLanguages: [],
  },
  {
    key: "sherpa-onnx-paraformer-large",
    family: LOCAL_MODEL_PARAFORMER,
    displayName: "Paraformer-zh-large",
    modelId: "sherpa-onnx-paraformer-zh-2025-10-07",
    sizeLabelKey: "sensevoice.modelSize.paraformerLarge",
    supportedLanguages: [],
  },
  {
    key: LOCAL_MODEL_FIRERED,
    family: LOCAL_MODEL_FIRERED,
    displayName: "FireRedASR2-CTC",
    modelId: "sherpa-onnx-fire-red-asr2-ctc-zh_en-int8-2026-02-25",
    sizeLabelKey: "sensevoice.modelSize.fireRedCtc",
    supportedLanguages: [],
  },
  {
    key: "sherpa-onnx-firered-large",
    family: LOCAL_MODEL_FIRERED,
    displayName: "FireRedASR-Large",
    modelId: "sherpa-onnx-fire-red-asr-large-zh_en-2025-02-16",
    sizeLabelKey: "sensevoice.modelSize.fireRedLarge",
    supportedLanguages: [],
  },
  {
    key: LOCAL_MODEL_FUNASR_NANO,
    family: LOCAL_MODEL_FUNASR_NANO,
    displayName: "Fun-ASR-Nano",
    modelId: "sherpa-onnx-funasr-nano-int8-2025-12-30",
    sizeLabelKey: "sensevoice.modelSize.funasrNanoInt8",
    supportedLanguages: FUNASR_NANO_LANGUAGES,
  },
  {
    key: "sherpa-onnx-funasr-nano-fp32",
    family: LOCAL_MODEL_FUNASR_NANO,
    displayName: "Fun-ASR-Nano-FP32",
    modelId: "sherpa-onnx-funasr-nano-2025-12-30",
    sizeLabelKey: "sensevoice.modelSize.funasrNanoFp32",
    supportedLanguages: FUNASR_NANO_LANGUAGES,
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

/** 指定系列下的全部规模 */
export const modelSizesFor = (family: string) =>
  LOCAL_MODELS.filter((spec) => spec.family === family);

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
