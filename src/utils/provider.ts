import type { TranscriptionProvider } from "../types/settings";

// ── 提供商分类 ──────────────────────────────────────────────

export type ProviderCategory = "cloud" | "local";

/** 获取提供商所属大类 */
export function getProviderCategory(
  provider: TranscriptionProvider,
): ProviderCategory {
  switch (provider) {
    case "sensevoice":
      return "local";
    case "openai":
    case "volcengine":
    case "aliyun-asr":
    case "aliyun-paraformer":
      return "cloud";
  }
}

// ── 本地运行时分类 ──────────────────────────────────────────

export type LocalRuntime = "docker" | "native";

const LOCAL_MODEL_RUNTIME_MAP: Record<string, LocalRuntime> = {
  sensevoice: "docker",
  "sherpa-onnx-sensevoice": "native",
  voxtral: "docker",
  "qwen3-asr": "docker",
};

/** 获取本地模型的运行时类型 */
export function getLocalRuntime(localModel: string): LocalRuntime {
  return LOCAL_MODEL_RUNTIME_MAP[localModel] ?? "docker";
}

/** 判断提供商是否为本地提供商 */
export function isLocalProvider(provider: TranscriptionProvider): boolean {
  return getProviderCategory(provider) === "local";
}

/** 判断提供商是否为云端提供商 */
export function isCloudProvider(provider: TranscriptionProvider): boolean {
  return getProviderCategory(provider) === "cloud";
}
