import type { TFunction } from "i18next";
import { CustomSelect } from "../CustomSelect";
import { NumberWheelInput } from "../NumberWheelInput";
import { SettingsCard } from "../SettingsCard";
import type { SenseVoiceProgress, SenseVoiceStatus } from "../../hooks/useSenseVoice";
import type { Settings } from "../../types/settings";
import { parseList, listToString, normalizeAliyunRegion } from "../../utils";

const isAliyunProvider = (provider: Settings["provider"]) =>
  provider === "aliyun-asr" || provider === "aliyun-paraformer";

interface Option {
  value: string;
  label: string;
}

interface SpeechSettingsSectionProps {
  draft: Settings;
  t: TFunction;
  updateDraft: (updater: (prev: Settings) => Settings) => void;
  supportsSherpaOnnxSenseVoice: boolean;
  sherpaFallbackActive: boolean;
  sensevoiceStatus: SenseVoiceStatus;
  sensevoiceProgress: SenseVoiceProgress | null;
  sensevoiceLogLines: string[];
  sensevoiceLogsExpanded: boolean;
  setSensevoiceLogsExpanded: (updater: (prev: boolean) => boolean) => void;
  sensevoiceLoading: boolean;
  handleSenseVoicePrepare: () => void;
  handleSenseVoiceStart: () => void;
  handleSenseVoiceStop: () => void;
  normalizeLocalModel: (value: string | undefined) => string;
  normalizeSenseVoiceLanguage: (value: string | undefined) => string;
  normalizeSenseVoiceDevice: (localModel: string | undefined, device: string | undefined) => string;
  normalizeStopMode: (value: string | undefined) => "stop" | "pause";
  isCudaOnlyLocalModel: (localModel: string | undefined) => boolean;
  getDefaultModelId: (localModel: string) => string;
  getQwenVariantByModelId: (modelId: string | undefined) => string;
  formatBytes: (value: number | undefined) => string;
  sherpaLanguageOptions: Option[];
  qwenVariantOptions: Option[];
}

export function SpeechSettingsSection({
  draft,
  t,
  updateDraft,
  supportsSherpaOnnxSenseVoice,
  sherpaFallbackActive,
  sensevoiceStatus,
  sensevoiceProgress,
  sensevoiceLogLines,
  sensevoiceLogsExpanded,
  setSensevoiceLogsExpanded,
  sensevoiceLoading,
  handleSenseVoicePrepare,
  handleSenseVoiceStart,
  handleSenseVoiceStop,
  normalizeLocalModel,
  normalizeSenseVoiceLanguage,
  normalizeSenseVoiceDevice,
  normalizeStopMode,
  isCudaOnlyLocalModel,
  getDefaultModelId,
  getQwenVariantByModelId,
  formatBytes,
  sherpaLanguageOptions,
  qwenVariantOptions,
}: SpeechSettingsSectionProps) {
  const localModelOptions: Option[] = [
    {
      value: "sensevoice",
      label: t("sensevoice.localModelSenseVoice"),
    },
    {
      value: "voxtral",
      label: t("sensevoice.localModelVoxtral"),
    },
    {
      value: "qwen3-asr",
      label: t("sensevoice.localModelQwen3Asr"),
    },
  ];
  if (supportsSherpaOnnxSenseVoice) {
    localModelOptions.splice(1, 0, {
      value: "sherpa-onnx-sensevoice",
      label: t("sensevoice.localModelSherpaOnnxSenseVoice"),
    });
  }

  return (
    <>
      <SettingsCard
        title={t("speech.title")}
        description={t("speech.description")}
      >
        <label className="field">
          <span>{t("speech.provider")}</span>
          <CustomSelect
            value={draft.provider}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                provider: value as Settings["provider"],
                aliyun: isAliyunProvider(value as Settings["provider"])
                  ? {
                      ...prev.aliyun,
                      region:
                        value === "aliyun-paraformer"
                          ? "beijing"
                          : normalizeAliyunRegion(prev.aliyun.region),
                    }
                  : prev.aliyun,
              }))
            }
            groups={[
              {
                label: t("speech.categoryCloud"),
                options: [
                  { value: "openai", label: "OpenAI" },
                  { value: "volcengine", label: t("speech.volcengine") },
                  { value: "aliyun-asr", label: t("speech.aliyunAsr") },
                  { value: "aliyun-paraformer", label: t("speech.aliyunParaformer") },
                ],
              },
              {
                label: t("speech.categoryLocal"),
                options: [
                  { value: "sensevoice", label: t("speech.sensevoice") },
                ],
              },
            ]}
          />
        </label>
      </SettingsCard>

      {draft.provider === "openai" ? (
        <SettingsCard title="OpenAI">
          <label className="field">
            <span>{t("openai.apiBase")}</span>
            <input
              value={draft.openai.apiBase}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: { ...prev.openai, apiBase: event.target.value },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("openai.apiKey")}</span>
            <input
              type="password"
              value={draft.openai.apiKey}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: { ...prev.openai, apiKey: event.target.value },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.model")}</span>
            <input
              value={draft.openai.speechToText.model}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      model: event.target.value,
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.language")}</span>
            <input
              value={draft.openai.speechToText.language}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      language: event.target.value,
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.prompt")}</span>
            <input
              value={draft.openai.speechToText.prompt}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      prompt: event.target.value,
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.responseFormat")}</span>
            <input
              value={draft.openai.speechToText.responseFormat}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      responseFormat: event.target.value,
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.temperature")}</span>
            <NumberWheelInput
              step={0.1}
              value={draft.openai.speechToText.temperature}
              onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      temperature: value,
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.chunkingStrategy")}</span>
            <input
              value={draft.openai.speechToText.chunkingStrategy}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      chunkingStrategy: event.target.value,
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.timestampGranularities")}</span>
            <input
              value={listToString(draft.openai.speechToText.timestampGranularities)}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      timestampGranularities: parseList(event.target.value),
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.include")}</span>
            <input
              value={listToString(draft.openai.speechToText.include)}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      include: parseList(event.target.value),
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.knownSpeakerNames")}</span>
            <input
              value={listToString(draft.openai.speechToText.knownSpeakerNames)}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      knownSpeakerNames: parseList(event.target.value),
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("speech.knownSpeakerReferences")}</span>
            <input
              value={listToString(draft.openai.speechToText.knownSpeakerReferences)}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      knownSpeakerReferences: parseList(event.target.value),
                    },
                  },
                }))
              }
            />
          </label>
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.openai.speechToText.stream}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      stream: event.target.checked,
                    },
                  },
                }))
              }
            />
            <span>{t("speech.stream")}</span>
          </label>
        </SettingsCard>
      ) : null}

      {draft.provider === "volcengine" ? (
        <SettingsCard title={t("speech.volcengine")}>
          <label className="field">
            <span>{t("volcengine.appId")}</span>
            <input
              value={draft.volcengine.appId}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, appId: event.target.value },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("volcengine.accessToken")}</span>
            <input
              type="password"
              value={draft.volcengine.accessToken}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, accessToken: event.target.value },
                }))
              }
            />
          </label>
          <label className="field">
            <span>{t("volcengine.language")}</span>
            <CustomSelect
              value={draft.volcengine.language}
              onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, language: value },
                }))
              }
              options={[
                { value: "zh-CN", label: t("volcengine.langZhCN") },
                { value: "zh-TW", label: t("volcengine.langZhTW") },
                { value: "en-US", label: t("volcengine.langEnUS") },
                { value: "ja-JP", label: t("volcengine.langJaJP") },
                { value: "ko-KR", label: t("volcengine.langKoKR") },
              ]}
            />
          </label>
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.volcengine.useStreaming}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, useStreaming: event.target.checked },
                }))
              }
            />
            <span>{t("volcengine.useStreaming")}</span>
          </label>
          <label className="field checkbox">
            <input
              type="checkbox"
              checked={draft.volcengine.useFast}
              onChange={(event) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, useFast: event.target.checked },
                }))
              }
            />
            <span>{t("volcengine.useFast")}</span>
          </label>
        </SettingsCard>
      ) : null}

      {draft.provider === "aliyun-asr" || draft.provider === "aliyun-paraformer" ? (
        <SettingsCard
          title={
            draft.provider === "aliyun-asr"
              ? t("speech.aliyunAsr")
              : t("speech.aliyunParaformer")
          }
        >
          {(() => {
            const isParaformer = draft.provider === "aliyun-paraformer";
            const selectedRegion = isParaformer
              ? "beijing"
              : normalizeAliyunRegion(draft.aliyun.region);
            const regionApiKey =
              selectedRegion === "singapore"
                ? draft.aliyun.apiKeys.singapore
                : draft.aliyun.apiKeys.beijing;
            return (
              <>
                <label className="field">
                  <span>{t("speech.region")}</span>
                  <CustomSelect
                    value={selectedRegion}
                    onChange={(value) =>
                      updateDraft((prev) => ({
                        ...prev,
                        aliyun: {
                          ...prev.aliyun,
                          region: isParaformer
                            ? "beijing"
                            : normalizeAliyunRegion(value),
                        },
                      }))
                    }
                    disabled={isParaformer}
                    options={[
                      { value: "beijing", label: t("aliyun.regionBeijing") },
                      { value: "singapore", label: t("aliyun.regionSingapore") },
                    ]}
                  />
                </label>
                {isParaformer ? (
                  <div className="sensevoice-hint">{t("aliyun.paraformerRegionHint")}</div>
                ) : null}
                <label className="field">
                  <span>{t("aliyun.apiKey")}</span>
                  <input
                    type="password"
                    value={regionApiKey}
                    onChange={(event) =>
                      updateDraft((prev) => {
                        const region = isParaformer
                          ? "beijing"
                          : normalizeAliyunRegion(prev.aliyun.region);
                        return {
                          ...prev,
                          aliyun: {
                            ...prev.aliyun,
                            apiKeys: {
                              ...prev.aliyun.apiKeys,
                              [region]: event.target.value,
                            },
                          },
                        };
                      })
                    }
                  />
                </label>
                {isParaformer ? (
                  <>
                    <label className="field">
                      <span>{t("aliyun.languageHints")}</span>
                      <input
                        value={listToString(draft.aliyun.paraformer.languageHints)}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            aliyun: {
                              ...prev.aliyun,
                              paraformer: {
                                ...prev.aliyun.paraformer,
                                languageHints: parseList(event.target.value),
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("aliyun.vocabularyId")}</span>
                      <input
                        value={draft.aliyun.paraformer.vocabularyId}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            aliyun: {
                              ...prev.aliyun,
                              paraformer: {
                                ...prev.aliyun.paraformer,
                                vocabularyId: event.target.value,
                              },
                            },
                          }))
                        }
                      />
                    </label>
                  </>
                ) : (
                  <label className="field">
                    <span>{t("aliyun.vocabularyId")}</span>
                    <input
                      value={draft.aliyun.asr.vocabularyId}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          aliyun: {
                            ...prev.aliyun,
                            asr: {
                              ...prev.aliyun.asr,
                              vocabularyId: event.target.value,
                            },
                          },
                        }))
                      }
                    />
                  </label>
                )}
              </>
            );
          })()}
        </SettingsCard>
      ) : null}

      {draft.provider === "sensevoice" ? (
        <SettingsCard title={t("speech.sensevoice")}>
          {(() => {
            const installed = sensevoiceStatus.installed;
            const running = sensevoiceStatus.running;
            const runtimeState = sensevoiceStatus.runtimeState || "stopped";
            const runtimeKind = sensevoiceStatus.runtimeKind || "docker";
            const supportsPause = sensevoiceStatus.supportsPause ?? true;
            const state = sensevoiceStatus.downloadState || draft.sensevoice.downloadState;
            const lastError = sensevoiceStatus.lastError || draft.sensevoice.lastError;
            const progressStage = sensevoiceProgress?.stage ?? "";
            const isReady = state === "ready";
            const isLoaded = state === "loaded";
            const isNativeRuntime = runtimeKind === "native";
            const isWarmupStage =
              progressStage === "verify" || progressStage === "warmup";
            const effectiveProgressStage =
              (isReady || isLoaded) && isWarmupStage ? "done" : progressStage;
            const isWarming =
              !isNativeRuntime &&
              !isReady &&
              (isWarmupStage || (running && state === "running"));
            const showProgressBar =
              !!sensevoiceProgress &&
              (effectiveProgressStage === "prepare" ||
                effectiveProgressStage === "install" ||
                effectiveProgressStage === "download" ||
                effectiveProgressStage === "loading");
            const stageLabelKey =
              isNativeRuntime && effectiveProgressStage === "loading"
                ? "loading"
                : effectiveProgressStage === "verify"
                  ? "started"
                  : effectiveProgressStage === "warmup"
                    ? "warmup"
                    : effectiveProgressStage === "resuming"
                      ? "resuming"
                      : effectiveProgressStage === "paused"
                        ? "paused"
                        : effectiveProgressStage === "done"
                          ? "ready"
                          : effectiveProgressStage === "error"
                            ? "error"
                            : isNativeRuntime && state === "loaded"
                              ? "loaded"
                              : runtimeState === "paused"
                                ? "paused"
                                : isWarming
                                  ? "warmup"
                                  : "";
            const prepareBusy =
              sensevoiceLoading ||
              effectiveProgressStage === "prepare" ||
              effectiveProgressStage === "install" ||
              effectiveProgressStage === "download" ||
              effectiveProgressStage === "loading";
            const startBusy =
              sensevoiceLoading ||
              effectiveProgressStage === "prepare" ||
              effectiveProgressStage === "install" ||
              effectiveProgressStage === "download" ||
              effectiveProgressStage === "loading" ||
              (running && !isReady && isWarmupStage);
            const stopBusy = sensevoiceLoading;
            const selectedLocalModel = normalizeLocalModel(draft.sensevoice.localModel);
            const isSherpaSelected = selectedLocalModel === "sherpa-onnx-sensevoice";
            const isVoxtralSelected = selectedLocalModel === "voxtral";
            const isQwenSelected = selectedLocalModel === "qwen3-asr";
            const isCudaOnlySelected = isCudaOnlyLocalModel(selectedLocalModel);
            const currentDevice = normalizeSenseVoiceDevice(
              selectedLocalModel,
              draft.sensevoice.device
            );
            const stopMode = normalizeStopMode(draft.sensevoice.stopMode);
            const selectedQwenVariant = getQwenVariantByModelId(draft.sensevoice.modelId);

            return (
              <>
                <div className="sensevoice-summary">
                  <span>
                    {t("sensevoice.installed")}:{" "}
                    {installed ? t("sensevoice.yes") : t("sensevoice.no")}
                  </span>
                  <span>
                    {t("sensevoice.running")}:{" "}
                    {isWarming
                      ? t("sensevoice.warmingNow")
                      : runtimeState === "paused"
                        ? t("sensevoice.pausedNow")
                        : running
                          ? t("sensevoice.runningNow")
                          : t("sensevoice.stopped")}
                  </span>
                  <span>
                    {t("sensevoice.state")}:{" "}
                    {t(`sensevoice.stateMap.${state}`, { defaultValue: state })}
                  </span>
                </div>

                <label className="field">
                  <span>{t("sensevoice.localModel")}</span>
                  <CustomSelect
                    value={selectedLocalModel}
                    onChange={(value) =>
                      updateDraft((prev) => {
                        const nextLocalModel = normalizeLocalModel(value);
                        const nextDefaultModelId = getDefaultModelId(nextLocalModel);
                        const nextDevice = normalizeSenseVoiceDevice(
                          nextLocalModel,
                          prev.sensevoice.device
                        );
                        return {
                          ...prev,
                          sensevoice: {
                            ...prev.sensevoice,
                            localModel: nextLocalModel,
                            modelId: nextDefaultModelId,
                            language: normalizeSenseVoiceLanguage(prev.sensevoice.language),
                            device: nextDevice,
                          },
                        };
                      })
                    }
                    options={localModelOptions}
                  />
                </label>

                {sherpaFallbackActive ? (
                  <div className="sensevoice-hint">
                    {t("sensevoice.sherpaUnsupportedFallbackHint")}
                  </div>
                ) : null}

                {isSherpaSelected ? (
                  <label className="field">
                    <span>{t("sensevoice.language")}</span>
                    <CustomSelect
                      value={normalizeSenseVoiceLanguage(draft.sensevoice.language)}
                      onChange={(value) =>
                        updateDraft((prev) => ({
                          ...prev,
                          sensevoice: {
                            ...prev.sensevoice,
                            language: normalizeSenseVoiceLanguage(value),
                          },
                        }))
                      }
                      options={sherpaLanguageOptions}
                    />
                  </label>
                ) : null}

                {isQwenSelected ? (
                  <label className="field">
                    <span>{t("sensevoice.qwenVariant")}</span>
                    <CustomSelect
                      value={selectedQwenVariant}
                      onChange={(value) =>
                        updateDraft((prev) => ({
                          ...prev,
                          sensevoice: {
                            ...prev.sensevoice,
                            modelId: value,
                          },
                        }))
                      }
                      options={qwenVariantOptions}
                    />
                  </label>
                ) : null}

                {!isSherpaSelected ? (
                  <label className="field">
                    <span>{t("sensevoice.serviceUrl")}</span>
                    <input
                      value={draft.sensevoice.serviceUrl}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          sensevoice: {
                            ...prev.sensevoice,
                            serviceUrl: event.target.value,
                          },
                        }))
                      }
                    />
                  </label>
                ) : null}

                <label className="field">
                  <span>{t("sensevoice.device")}</span>
                  <CustomSelect
                    value={currentDevice}
                    onChange={(value) =>
                      updateDraft((prev) => ({
                        ...prev,
                        sensevoice: {
                          ...prev.sensevoice,
                          device: normalizeSenseVoiceDevice(prev.sensevoice.localModel, value),
                        },
                      }))
                    }
                    disabled={isCudaOnlySelected || isSherpaSelected}
                    options={[
                      { value: "auto", label: t("sensevoice.deviceAuto") },
                      { value: "cpu", label: t("sensevoice.deviceCpu") },
                      { value: "cuda", label: t("sensevoice.deviceCuda") },
                    ]}
                  />
                </label>

                {!isSherpaSelected && supportsPause ? (
                  <>
                    <label className="field">
                      <span>{t("sensevoice.stopMode")}</span>
                      <CustomSelect
                        value={stopMode}
                        onChange={(value) =>
                          updateDraft((prev) => ({
                            ...prev,
                            sensevoice: {
                              ...prev.sensevoice,
                              stopMode: normalizeStopMode(value),
                            },
                          }))
                        }
                        options={[
                          { value: "stop", label: t("sensevoice.stopModeStop") },
                          { value: "pause", label: t("sensevoice.stopModePause") },
                        ]}
                      />
                    </label>
                    <div className="sensevoice-hint">{t("sensevoice.stopModeHint")}</div>
                  </>
                ) : null}
                {isVoxtralSelected ? (
                  <div className="sensevoice-hint">{t("sensevoice.voxtralCudaOnlyHint")}</div>
                ) : null}
                {isSherpaSelected ? (
                  <div className="sensevoice-hint">{t("sensevoice.sherpaCpuOnlyHint")}</div>
                ) : null}
                {isQwenSelected ? (
                  <div className="sensevoice-hint">{t("sensevoice.qwenCudaOnlyHint")}</div>
                ) : null}

                {showProgressBar ? (
                  <div className="sensevoice-progress">
                    <span>{sensevoiceProgress?.message}</span>
                    <span>
                      {sensevoiceProgress?.percent !== undefined
                        ? `${sensevoiceProgress.percent}%`
                        : ""}
                    </span>
                  </div>
                ) : null}

                {showProgressBar &&
                (sensevoiceProgress?.downloadedBytes !== undefined ||
                  sensevoiceProgress?.totalBytes !== undefined) ? (
                  <div className="sensevoice-hint">
                    {sensevoiceProgress?.totalBytes !== undefined
                      ? `${formatBytes(sensevoiceProgress?.downloadedBytes)} / ${formatBytes(
                          sensevoiceProgress?.totalBytes
                        )}`
                      : formatBytes(sensevoiceProgress?.downloadedBytes)}
                  </div>
                ) : null}

                {sensevoiceProgress?.stage === "install" ? (
                  <div className="sensevoice-hint">{t("sensevoice.installingHint")}</div>
                ) : null}

                {stageLabelKey ? (
                  <div className="sensevoice-hint">
                    {t(`sensevoice.stageStatus.${stageLabelKey}`)}
                  </div>
                ) : null}

                {import.meta.env.DEV ? (
                  <div className="sensevoice-hint">{t("sensevoice.devConsoleHint")}</div>
                ) : null}

                {sensevoiceLogLines.length > 0 ? (
                  <div className="sensevoice-log">
                    <div className="sensevoice-log-header">
                      <div className="sensevoice-log-title">{t("sensevoice.logTitle")}</div>
                      <button
                        type="button"
                        className="sensevoice-log-toggle"
                        onClick={() => setSensevoiceLogsExpanded((prev) => !prev)}
                      >
                        {sensevoiceLogsExpanded
                          ? t("sensevoice.logCollapse")
                          : t("sensevoice.logExpand")}
                      </button>
                    </div>
                    {sensevoiceLogsExpanded ? <pre>{sensevoiceLogLines.join("\n")}</pre> : null}
                  </div>
                ) : null}

                {lastError ? (
                  <>
                    <div className="sensevoice-error">{lastError}</div>
                    <div className="sensevoice-hint">{t("sensevoice.serverLogHint")}</div>
                  </>
                ) : null}

                <div className="button-row">
                  {!installed ? (
                    <button
                      type="button"
                      onClick={handleSenseVoicePrepare}
                      disabled={prepareBusy}
                    >
                      {t("sensevoice.prepare")}
                    </button>
                  ) : null}
                  {installed && !running ? (
                    <button type="button" onClick={handleSenseVoiceStart} disabled={startBusy}>
                      {isNativeRuntime ? t("sensevoice.load") : t("sensevoice.start")}
                    </button>
                  ) : null}
                  {running ? (
                    <button type="button" onClick={handleSenseVoiceStop} disabled={stopBusy}>
                      {isNativeRuntime ? t("sensevoice.unload") : t("sensevoice.stop")}
                    </button>
                  ) : null}
                </div>
              </>
            );
          })()}
        </SettingsCard>
      ) : null}
    </>
  );
}
