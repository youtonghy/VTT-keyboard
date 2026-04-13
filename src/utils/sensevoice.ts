export const DEFAULT_SENSEVOICE_MODEL_ID = "FunAudioLLM/SenseVoiceSmall";
export const DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID =
  "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
export const DEFAULT_VOXTRAL_MODEL_ID = "mistralai/Voxtral-Mini-4B-Realtime-2602";
export const DEFAULT_QWEN3_ASR_MODEL_ID = "Qwen/Qwen3-ASR-1.7B";

export const SHERPA_LANGUAGE_OPTIONS = [
  { value: "auto", labelKey: "sensevoice.languageAuto" },
  { value: "zh", labelKey: "sensevoice.languageZh" },
  { value: "en", labelKey: "sensevoice.languageEn" },
  { value: "ja", labelKey: "sensevoice.languageJa" },
  { value: "ko", labelKey: "sensevoice.languageKo" },
  { value: "yue", labelKey: "sensevoice.languageYue" },
] as const;

export const QWEN3_ASR_MODEL_VARIANTS = [
  { value: "Qwen/Qwen3-ASR-1.7B", labelKey: "sensevoice.qwenVariant17b" },
  { value: "Qwen/Qwen3-ASR-0.6B", labelKey: "sensevoice.qwenVariant06b" },
  {
    value: "Qwen/Qwen3-ForcedAligner-0.6B",
    labelKey: "sensevoice.qwenVariantForcedAligner",
  },
] as const;

export const normalizeLocalModel = (value: string | undefined) => {
  if (value === "sherpa-onnx-sensevoice") {
    return "sherpa-onnx-sensevoice";
  }
  if (value === "voxtral") {
    return "voxtral";
  }
  if (value === "qwen3-asr") {
    return "qwen3-asr";
  }
  return "sensevoice";
};

export const normalizeStopMode = (value: string | undefined): "stop" | "pause" => {
  if (value === "pause") {
    return "pause";
  }
  return "stop";
};

export const isCudaOnlyLocalModel = (localModel: string | undefined) => {
  const normalized = normalizeLocalModel(localModel);
  return normalized === "voxtral" || normalized === "qwen3-asr";
};

export const isSherpaLocalModel = (localModel: string | undefined) =>
  normalizeLocalModel(localModel) === "sherpa-onnx-sensevoice";

export const normalizeSenseVoiceDevice = (
  localModel: string | undefined,
  device: string | undefined
) => {
  if (isCudaOnlyLocalModel(localModel)) {
    return "cuda";
  }
  if (isSherpaLocalModel(localModel)) {
    return "cpu";
  }
  if (device === "cpu" || device === "cuda") {
    return device;
  }
  return "auto";
};

export const getDefaultModelId = (localModel: string) => {
  const normalized = normalizeLocalModel(localModel);
  if (normalized === "sherpa-onnx-sensevoice") {
    return DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID;
  }
  if (normalized === "voxtral") {
    return DEFAULT_VOXTRAL_MODEL_ID;
  }
  if (normalized === "qwen3-asr") {
    return DEFAULT_QWEN3_ASR_MODEL_ID;
  }
  return DEFAULT_SENSEVOICE_MODEL_ID;
};

export const normalizeSenseVoiceModelId = (localModel: string, modelId: string | undefined) => {
  const normalized = normalizeLocalModel(localModel);
  if (normalized !== "qwen3-asr") {
    return getDefaultModelId(normalized);
  }
  const trimmed = modelId?.trim();
  if (!trimmed) {
    return DEFAULT_QWEN3_ASR_MODEL_ID;
  }
  const matched = QWEN3_ASR_MODEL_VARIANTS.find((option) => option.value === trimmed);
  return matched ? matched.value : DEFAULT_QWEN3_ASR_MODEL_ID;
};

export const normalizeSenseVoiceLanguage = (language: string | undefined) => {
  if (language === "zh" || language === "en" || language === "ja" || language === "ko" || language === "yue") {
    return language;
  }
  return "auto";
};

export const getQwenVariantByModelId = (modelId: string | undefined) => {
  const trimmed = modelId?.trim();
  if (!trimmed) {
    return DEFAULT_QWEN3_ASR_MODEL_ID;
  }
  const matched = QWEN3_ASR_MODEL_VARIANTS.find((option) => option.value === trimmed);
  return matched ? matched.value : DEFAULT_QWEN3_ASR_MODEL_ID;
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
