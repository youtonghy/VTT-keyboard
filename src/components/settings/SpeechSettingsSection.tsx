import type { TFunction } from "i18next";
import { Button } from "@heroui/react";
import { RefreshCw, RotateCw, Square, Trash2, Play, Pause } from "lucide-react";
import { CustomSelect } from "../CustomSelect";
import { HeroCheckboxField, HeroField, HeroInputField } from "../HeroField";
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
  sensevoiceDockerRuntimeStatus: {
    available: boolean;
    daemonRunning: boolean;
    containerName: string;
    containerExists: boolean;
    containerState: string;
    containerModelKey: string;
    containerModelId: string;
    expectedModelKey: string;
    expectedModelId: string;
    imageTag: string;
    imageExists: boolean;
    serviceUrl: string;
    runtimeDir: string;
    modelsDir: string;
    lastError: string;
  };
  sensevoiceProgress: SenseVoiceProgress | null;
  sensevoiceLogLines: string[];
  sensevoiceLogsExpanded: boolean;
  setSensevoiceLogsExpanded: (updater: (prev: boolean) => boolean) => void;
  sensevoiceLoading: boolean;
  handleSenseVoicePrepare: () => void;
  handleSenseVoiceStart: () => void;
  handleSenseVoiceStop: () => void;
  handleSenseVoiceRestart: () => void;
  handleSenseVoiceRemoveContainer: () => void;
  handleUpdateRuntime: () => void;
  refreshSenseVoiceStatus: () => Promise<unknown>;
  refreshSenseVoiceDockerRuntimeStatus: () => Promise<unknown>;
  normalizeLocalModel: (value: string | undefined) => string;
  normalizeSenseVoiceLanguage: (value: string | undefined) => string;
  normalizeSenseVoiceDevice: (localModel: string | undefined, device: string | undefined) => string;
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
  sensevoiceDockerRuntimeStatus,
  sensevoiceProgress,
  sensevoiceLogLines,
  sensevoiceLogsExpanded,
  setSensevoiceLogsExpanded,
  sensevoiceLoading,
  handleSenseVoicePrepare,
  handleSenseVoiceStart,
  handleSenseVoiceStop,
  handleSenseVoiceRestart,
  handleSenseVoiceRemoveContainer,
  handleUpdateRuntime,
  refreshSenseVoiceStatus,
  refreshSenseVoiceDockerRuntimeStatus,
  normalizeLocalModel,
  normalizeSenseVoiceLanguage,
  normalizeSenseVoiceDevice,
  isCudaOnlyLocalModel,
  getDefaultModelId,
  getQwenVariantByModelId,
  formatBytes,
  sherpaLanguageOptions,
  qwenVariantOptions,
}: SpeechSettingsSectionProps) {
  const privacyModeEnabled = draft.privacy.enabled;
  const providerGroups = privacyModeEnabled
    ? [
        {
          label: t("speech.categoryLocal"),
          options: [{ value: "sensevoice", label: t("speech.sensevoice") }],
        },
      ]
    : [
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
          options: [{ value: "sensevoice", label: t("speech.sensevoice") }],
        },
      ];
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
        <HeroField label={t("speech.provider")}><CustomSelect
            value={draft.provider}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                provider: privacyModeEnabled
                  ? "sensevoice"
                  : (value as Settings["provider"]),
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
            groups={providerGroups}
           /></HeroField>
        {privacyModeEnabled ? (
          <div className="sensevoice-hint">{t("speech.privacyModeLocalOnly")}</div>
        ) : null}
      </SettingsCard>

      {draft.provider === "openai" ? (
        <SettingsCard title="OpenAI">
          <HeroInputField
            label={t("openai.apiBase")}
            value={draft.openai.apiBase}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: { ...prev.openai, apiBase: value },
                }))
            }
          />
          <HeroInputField
            label={t("openai.apiKey")}
            value={draft.openai.apiKey}
            type="password"
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: { ...prev.openai, apiKey: value },
                }))
            }
          />
          <HeroInputField
            label={t("speech.model")}
            value={draft.openai.speechToText.model}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      model: value,
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.language")}
            value={draft.openai.speechToText.language}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      language: value,
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.prompt")}
            value={draft.openai.speechToText.prompt}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      prompt: value,
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.responseFormat")}
            value={draft.openai.speechToText.responseFormat}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      responseFormat: value,
                    },
                  },
                }))
            }
          />
          <HeroField label={t("speech.temperature")}><NumberWheelInput
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
             /></HeroField>
          <HeroInputField
            label={t("speech.chunkingStrategy")}
            value={draft.openai.speechToText.chunkingStrategy}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      chunkingStrategy: value,
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.timestampGranularities")}
            value={listToString(draft.openai.speechToText.timestampGranularities)}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      timestampGranularities: parseList(value),
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.include")}
            value={listToString(draft.openai.speechToText.include)}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      include: parseList(value),
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.knownSpeakerNames")}
            value={listToString(draft.openai.speechToText.knownSpeakerNames)}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      knownSpeakerNames: parseList(value),
                    },
                  },
                }))
            }
          />
          <HeroInputField
            label={t("speech.knownSpeakerReferences")}
            value={listToString(draft.openai.speechToText.knownSpeakerReferences)}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      knownSpeakerReferences: parseList(value),
                    },
                  },
                }))
            }
          />
          <HeroCheckboxField
            label={t("speech.stream")}
            isSelected={draft.openai.speechToText.stream}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: {
                    ...prev.openai,
                    speechToText: {
                      ...prev.openai.speechToText,
                      stream: value,
                    },
                  },
                }))
            }
          />
        </SettingsCard>
      ) : null}

      {draft.provider === "volcengine" ? (
        <SettingsCard title={t("speech.volcengine")}>
          <HeroInputField
            label={t("volcengine.appId")}
            value={draft.volcengine.appId}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, appId: value },
                }))
            }
          />
          <HeroInputField
            label={t("volcengine.accessToken")}
            value={draft.volcengine.accessToken}
            type="password"
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, accessToken: value },
                }))
            }
          />
          <HeroField label={t("volcengine.language")}><CustomSelect
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
             /></HeroField>
          <HeroCheckboxField
            label={t("volcengine.useStreaming")}
            isSelected={draft.volcengine.useStreaming}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, useStreaming: value },
                }))
            }
          />
          <HeroCheckboxField
            label={t("volcengine.useFast")}
            isSelected={draft.volcengine.useFast}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, useFast: value },
                }))
            }
          />
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
                <HeroField label={t("speech.region")}><CustomSelect
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
                   /></HeroField>
                {isParaformer ? (
                  <div className="sensevoice-hint">{t("aliyun.paraformerRegionHint")}</div>
                ) : null}
                <HeroInputField
            label={t("aliyun.apiKey")}
            value={regionApiKey}
            type="password"
            onChange={(value) =>
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
                              [region]: value,
                            },
                          },
                        };
                      })
            }
          />
                {isParaformer ? (
                  <>
                    <HeroInputField
            label={t("aliyun.languageHints")}
            value={listToString(draft.aliyun.paraformer.languageHints)}
            onChange={(value) =>
                          updateDraft((prev) => ({
                            ...prev,
                            aliyun: {
                              ...prev.aliyun,
                              paraformer: {
                                ...prev.aliyun.paraformer,
                                languageHints: parseList(value),
                              },
                            },
                          }))
            }
          />
                    <HeroInputField
            label={t("aliyun.vocabularyId")}
            value={draft.aliyun.paraformer.vocabularyId}
            onChange={(value) =>
                          updateDraft((prev) => ({
                            ...prev,
                            aliyun: {
                              ...prev.aliyun,
                              paraformer: {
                                ...prev.aliyun.paraformer,
                                vocabularyId: value,
                              },
                            },
                          }))
            }
          />
                  </>
                ) : (
                  <HeroInputField
            label={t("aliyun.vocabularyId")}
            value={draft.aliyun.asr.vocabularyId}
            onChange={(value) =>
                        updateDraft((prev) => ({
                          ...prev,
                          aliyun: {
                            ...prev.aliyun,
                            asr: {
                              ...prev.aliyun.asr,
                              vocabularyId: value,
                            },
                          },
                        }))
            }
          />
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
            const selectedQwenVariant = getQwenVariantByModelId(draft.sensevoice.modelId);
            const dockerRuntimeReady =
              sensevoiceDockerRuntimeStatus.available && sensevoiceDockerRuntimeStatus.daemonRunning;
            const dockerRuntimeState = sensevoiceDockerRuntimeStatus.containerState || "stopped";
            const dockerRuntimeStateLabel =
              dockerRuntimeState === "running"
                ? t("sensevoice.runtimeEnvironmentRunning")
                : dockerRuntimeState === "paused"
                  ? t("sensevoice.runtimeEnvironmentPaused")
                  : dockerRuntimeState === "exited" || dockerRuntimeState === "created"
                    ? t("sensevoice.runtimeEnvironmentStopped")
                    : dockerRuntimeReady && !sensevoiceDockerRuntimeStatus.containerExists
                      ? t("sensevoice.runtimeEnvironmentMissing")
                      : t("sensevoice.runtimeEnvironmentError");
            const dockerRuntimeModelLabel =
              sensevoiceDockerRuntimeStatus.containerModelId ||
              sensevoiceDockerRuntimeStatus.expectedModelId ||
              draft.sensevoice.modelId;
            const runtimePanelBusy =
              sensevoiceLoading ||
              progressStage === "prepare" ||
              progressStage === "install" ||
              progressStage === "download" ||
              progressStage === "loading";

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

                <HeroField label={t("sensevoice.localModel")}><CustomSelect
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
                   /></HeroField>

                {sherpaFallbackActive ? (
                  <div className="sensevoice-hint">
                    {t("sensevoice.sherpaUnsupportedFallbackHint")}
                  </div>
                ) : null}

                {isSherpaSelected ? (
                  <HeroField label={t("sensevoice.language")}><CustomSelect
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
                     /></HeroField>
                ) : null}

                {isQwenSelected ? (
                  <HeroField label={t("sensevoice.qwenVariant")}><CustomSelect
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
                     /></HeroField>
                ) : null}

                {!isSherpaSelected ? (
                  <HeroInputField
            label={t("sensevoice.serviceUrl")}
            value={draft.sensevoice.serviceUrl}
            onChange={(value) =>
                        updateDraft((prev) => ({
                          ...prev,
                          sensevoice: {
                            ...prev.sensevoice,
                            serviceUrl: value,
                          },
                        }))
            }
          />
                ) : null}

                <HeroField label={t("sensevoice.device")}><CustomSelect
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
                   /></HeroField>

                {isVoxtralSelected ? (
                  <div className="sensevoice-hint">{t("sensevoice.voxtralCudaOnlyHint")}</div>
                ) : null}
                {isSherpaSelected ? (
                  <div className="sensevoice-hint">{t("sensevoice.sherpaCpuOnlyHint")}</div>
                ) : null}
                {isQwenSelected ? (
                  <div className="sensevoice-hint">{t("sensevoice.qwenCudaOnlyHint")}</div>
                ) : null}

                {!isNativeRuntime ? (
                  <div className="sensevoice-runtime-panel">
                    <div className="sensevoice-runtime-panel-header">
                      <div>
                        <div className="sensevoice-runtime-panel-title">
                          {t("sensevoice.runtimePanelTitle")}
                        </div>
                        <div className="sensevoice-runtime-panel-description">
                          {t("sensevoice.runtimePanelDescription")}
                        </div>
                      </div>
                      <Button
                        type="button"
                        variant="tertiary"
                        isIconOnly
                        isDisabled={runtimePanelBusy}
                        onPress={() => {
                          void refreshSenseVoiceStatus();
                          void refreshSenseVoiceDockerRuntimeStatus();
                        }}
                        aria-label={t("sensevoice.runtimeRefresh")}
                      >
                        <RefreshCw size={16} />
                      </Button>
                    </div>

                    <div className="sensevoice-runtime-grid">
                      <div>
                        <span>{t("sensevoice.runtimeStatus")}</span>
                        <strong>{dockerRuntimeStateLabel}</strong>
                      </div>
                      <div>
                        <span>{t("sensevoice.runtimeContainer")}</span>
                        <strong>{sensevoiceDockerRuntimeStatus.containerName || "—"}</strong>
                      </div>
                      <div>
                        <span>{t("sensevoice.runtimeImage")}</span>
                        <strong>{sensevoiceDockerRuntimeStatus.imageTag || "—"}</strong>
                      </div>
                      <div>
                        <span>{t("sensevoice.runtimeModel")}</span>
                        <strong>{dockerRuntimeModelLabel}</strong>
                      </div>
                    </div>

                    {sensevoiceDockerRuntimeStatus.lastError ? (
                      <div className="sensevoice-error">
                        {sensevoiceDockerRuntimeStatus.lastError}
                      </div>
                    ) : null}

                    {!dockerRuntimeReady ? (
                      <div className="sensevoice-hint">
                        {t("sensevoice.runtimeEnvironmentError")}
                      </div>
                    ) : null}

                    <div className="button-row sensevoice-runtime-actions">
                      {installed && !running ? (
                        <Button
                          type="button"
                          variant="primary"
                          onPress={handleSenseVoiceStart}
                          isDisabled={startBusy}
                        >
                          <Play size={16} />
                          {isNativeRuntime ? t("sensevoice.load") : t("sensevoice.runtimeStart")}
                        </Button>
                      ) : null}
                      {running ? (
                        <Button
                          type="button"
                          variant="secondary"
                          onPress={handleSenseVoiceStop}
                          isDisabled={stopBusy}
                        >
                          {draft.sensevoice.stopMode === "pause" ? <Pause size={16} /> : <Square size={16} />}
                          {isNativeRuntime ? t("sensevoice.unload") : draft.sensevoice.stopMode === "pause" ? t("sensevoice.runtimePause") : t("sensevoice.runtimeStop")}
                        </Button>
                      ) : null}
                      {!isNativeRuntime ? (
                        <>
                          <Button
                            type="button"
                            variant="secondary"
                            onPress={handleSenseVoiceRestart}
                            isDisabled={runtimePanelBusy || !installed}
                          >
                            <RotateCw size={16} />
                            {t("sensevoice.runtimeRestart")}
                          </Button>
                          <Button
                            type="button"
                            variant="secondary"
                            onPress={handleSenseVoiceRemoveContainer}
                            isDisabled={runtimePanelBusy || !installed}
                          >
                            <Trash2 size={16} />
                            {t("sensevoice.runtimeRemove")}
                          </Button>
                        </>
                      ) : null}
                    </div>
                  </div>
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
                      <Button
                        type="button"
                        className="sensevoice-log-toggle"
                        variant="secondary"
                        onPress={() => setSensevoiceLogsExpanded((prev) => !prev)}
                      >
                        {sensevoiceLogsExpanded
                          ? t("sensevoice.logCollapse")
                          : t("sensevoice.logExpand")}
                      </Button>
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
                    <Button
                      type="button"
                      variant="primary"
                      onPress={handleSenseVoicePrepare}
                      isDisabled={prepareBusy}
                      >
                        {t("sensevoice.prepare")}
                      </Button>
                  ) : null}
                  {installed && !running && !isNativeRuntime ? (
                    <Button
                      type="button"
                      variant="secondary"
                      onPress={handleUpdateRuntime}
                      isDisabled={sensevoiceLoading}
                    >
                      {t("sensevoice.updateRuntime")}
                    </Button>
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
