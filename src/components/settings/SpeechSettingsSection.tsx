import type { TFunction } from "i18next";
import { Button } from "@/components/motion/button/base";
import { RefreshCw, Download, Square, Trash2, Play } from "lucide-react";
import { CustomSelect } from "../CustomSelect";
import { ToggleField, Field, InputField } from "../FormFields";
import { NumberWheelInput } from "../NumberWheelInput";
import { SettingsCard } from "../SettingsCard";
import type { SenseVoiceProgress, SenseVoiceStatus } from "../../hooks/useSenseVoice";
import type { Settings } from "../../types/settings";
import { parseList, listToString, normalizeAliyunRegion } from "../../utils";
import {
  LOCAL_MODELS,
  LANGUAGE_OPTIONS,
  normalizeLocalModel,
  normalizeSenseVoiceLanguage,
  languageOptionsFor,
  getDefaultModelId,
  formatBytes,
} from "../../utils/sensevoice";

const isAliyunProvider = (provider: Settings["provider"]) =>
  provider === "aliyun-asr" || provider === "aliyun-paraformer";

interface SpeechSettingsSectionProps {
  draft: Settings;
  t: TFunction;
  updateDraft: (updater: (prev: Settings) => Settings) => void;
  sensevoiceStatus: SenseVoiceStatus;
  sensevoiceProgress: SenseVoiceProgress | null;
  sensevoiceLogLines: string[];
  sensevoiceLogsExpanded: boolean;
  setSensevoiceLogsExpanded: (updater: (prev: boolean) => boolean) => void;
  sensevoiceLoading: boolean;
  handleSenseVoicePrepare: () => void;
  handleSenseVoiceStart: () => void;
  handleSenseVoiceStop: () => void;
  handleSenseVoiceRemoveModel: () => void;
  handleSenseVoiceConfigSave: (patch: {
    enabled?: boolean;
    installed?: boolean;
    localModel?: string;
    modelId?: string;
    language?: string;
  }) => Promise<void>;
  refreshSenseVoiceStatus: () => Promise<unknown>;
}

export function SpeechSettingsSection({
  draft,
  t,
  updateDraft,
  sensevoiceStatus,
  sensevoiceProgress,
  sensevoiceLogLines,
  sensevoiceLogsExpanded,
  setSensevoiceLogsExpanded,
  sensevoiceLoading,
  handleSenseVoicePrepare,
  handleSenseVoiceStart,
  handleSenseVoiceStop,
  handleSenseVoiceRemoveModel,
  handleSenseVoiceConfigSave,
  refreshSenseVoiceStatus,
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
  const localModelOptions = LOCAL_MODELS.map((spec) => ({
    value: spec.key,
    label: t(spec.labelKey),
  }));

  return (
    <>
      <SettingsCard
        title={t("speech.title")}
        description={t("speech.description")}
      >
        <Field label={t("speech.provider")}><CustomSelect
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
           /></Field>
        {privacyModeEnabled ? (
          <div className="text-muted-foreground text-xs leading-5">{t("speech.privacyModeLocalOnly")}</div>
        ) : null}
      </SettingsCard>

      {draft.provider === "openai" ? (
        <SettingsCard title="OpenAI">
          <InputField
            label={t("openai.apiBase")}
            value={draft.openai.apiBase}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  openai: { ...prev.openai, apiBase: value },
                }))
            }
          />
          <InputField
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
          <InputField
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
          <InputField
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
          <InputField
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
          <InputField
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
          <Field label={t("speech.temperature")}><NumberWheelInput
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
             /></Field>
          <InputField
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
          <InputField
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
          <InputField
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
          <InputField
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
          <InputField
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
          <ToggleField
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
          <InputField
            label={t("volcengine.appId")}
            value={draft.volcengine.appId}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, appId: value },
                }))
            }
          />
          <InputField
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
          <Field label={t("volcengine.language")}><CustomSelect
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
             /></Field>
          <ToggleField
            label={t("volcengine.useStreaming")}
            isSelected={draft.volcengine.useStreaming}
            onChange={(value) =>
                updateDraft((prev) => ({
                  ...prev,
                  volcengine: { ...prev.volcengine, useStreaming: value },
                }))
            }
          />
          <ToggleField
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
                <Field label={t("speech.region")}><CustomSelect
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
                   /></Field>
                {isParaformer ? (
                  <div className="text-muted-foreground text-xs leading-5">{t("aliyun.paraformerRegionHint")}</div>
                ) : null}
                <InputField
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
                    <InputField
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
                    <InputField
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
                  <InputField
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
            const state = sensevoiceStatus.downloadState || draft.sensevoice.downloadState;
            const lastError = sensevoiceStatus.lastError || draft.sensevoice.lastError;
            const progressStage = sensevoiceProgress?.stage ?? "";
            const showProgressBar =
              !!sensevoiceProgress &&
              (progressStage === "prepare" ||
                progressStage === "download" ||
                progressStage === "loading");
            const stageLabelKey =
              progressStage === "loading"
                ? "loading"
                : progressStage === "done"
                  ? "ready"
                  : progressStage === "error"
                    ? "error"
                    : state === "loaded"
                      ? "loaded"
                      : "";
            const busy =
              sensevoiceLoading ||
              progressStage === "prepare" ||
              progressStage === "download" ||
              progressStage === "loading";
            const selectedLocalModel = normalizeLocalModel(draft.sensevoice.localModel);
            const languageValues = languageOptionsFor(selectedLocalModel);
            const languageOptions = LANGUAGE_OPTIONS.filter((option) =>
              languageValues.includes(option.value)
            ).map((option) => ({ value: option.value, label: t(option.labelKey) }));

            return (
              <>
                <div className="flex flex-wrap gap-2 text-xs [&>span]:rounded-full [&>span]:bg-muted [&>span]:px-3 [&>span]:py-1.5 [&>span]:text-muted-foreground">
                  <span>
                    {t("sensevoice.installed")}:{" "}
                    {installed ? t("sensevoice.yes") : t("sensevoice.no")}
                  </span>
                  <span>
                    {t("sensevoice.running")}:{" "}
                    {running ? t("sensevoice.runningNow") : t("sensevoice.stopped")}
                  </span>
                  <span>
                    {t("sensevoice.state")}:{" "}
                    {t(`sensevoice.stateMap.${state}`, { defaultValue: state })}
                  </span>
                </div>

                <Field label={t("sensevoice.localModel")}><CustomSelect
                    value={selectedLocalModel}
                    disabled={busy}
                    onChange={(value) => {
                      const nextLocalModel = normalizeLocalModel(value);
                      const nextLanguage = normalizeSenseVoiceLanguage(
                        draft.sensevoice.language,
                        nextLocalModel
                      );
                      const nextModelId = getDefaultModelId(nextLocalModel);
                      updateDraft((prev) => ({
                        ...prev,
                        sensevoice: {
                          ...prev.sensevoice,
                          localModel: nextLocalModel,
                          modelId: nextModelId,
                          language: nextLanguage,
                        },
                      }));
                      // 点击选择后下载：切换模型后立即保存配置并按需下载
                      void handleSenseVoiceConfigSave({
                        localModel: nextLocalModel,
                        modelId: nextModelId,
                        language: nextLanguage,
                      });
                    }}
                    options={localModelOptions}
                   /></Field>

                {languageOptions.length > 0 ? (
                  <Field label={t("sensevoice.language")}><CustomSelect
                      value={normalizeSenseVoiceLanguage(
                        draft.sensevoice.language,
                        selectedLocalModel
                      )}
                      onChange={(value) => {
                        const nextLanguage = normalizeSenseVoiceLanguage(
                          value,
                          selectedLocalModel
                        );
                        updateDraft((prev) => ({
                          ...prev,
                          sensevoice: {
                            ...prev.sensevoice,
                            language: nextLanguage,
                          },
                        }));
                        void handleSenseVoiceConfigSave({ language: nextLanguage });
                      }}
                      options={languageOptions}
                     /></Field>
                ) : null}

                <div className="text-muted-foreground text-xs leading-5">{t("sensevoice.nativeCpuHint")}</div>

                {showProgressBar ? (
                  <div className="flex items-center justify-between gap-3 rounded-2xl bg-muted px-4 py-3 text-sm">
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
                  <div className="text-muted-foreground text-xs leading-5">
                    {sensevoiceProgress?.totalBytes !== undefined
                      ? `${formatBytes(sensevoiceProgress?.downloadedBytes)} / ${formatBytes(
                          sensevoiceProgress?.totalBytes
                        )}`
                      : formatBytes(sensevoiceProgress?.downloadedBytes)}
                  </div>
                ) : null}

                {progressStage === "prepare" || progressStage === "download" ? (
                  <div className="text-muted-foreground text-xs leading-5">{t("sensevoice.installingHint")}</div>
                ) : null}

                {stageLabelKey ? (
                  <div className="text-muted-foreground text-xs leading-5">
                    {t(`sensevoice.stageStatus.${stageLabelKey}`)}
                  </div>
                ) : null}

                {import.meta.env.DEV ? (
                  <div className="text-muted-foreground text-xs leading-5">{t("sensevoice.devConsoleHint")}</div>
                ) : null}

                {sensevoiceLogLines.length > 0 ? (
                  <div className="grid gap-2 rounded-2xl border border-border p-3">
                    <div className="flex items-center justify-between gap-3">
                      <div className="font-medium text-sm">{t("sensevoice.logTitle")}</div>
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        onClick={() => setSensevoiceLogsExpanded((prev) => !prev)}
                      >
                        {sensevoiceLogsExpanded
                          ? t("sensevoice.logCollapse")
                          : t("sensevoice.logExpand")}
                      </Button>
                    </div>
                    {sensevoiceLogsExpanded ? <pre className="max-h-60 overflow-auto rounded-xl bg-muted p-3 font-mono text-[11px] leading-5 whitespace-pre-wrap">{sensevoiceLogLines.join("\n")}</pre> : null}
                  </div>
                ) : null}

                {lastError ? <div className="rounded-2xl bg-destructive/10 px-4 py-3 text-destructive text-sm">{lastError}</div> : null}

                <div className="flex flex-wrap items-center gap-2">
                  {!installed ? (
                    <Button
                      type="button"
                      variant="primary"
                      onClick={handleSenseVoicePrepare}
                      disabled={busy}
                    >
                      <Download size={16} />
                      {t("sensevoice.prepare")}
                    </Button>
                  ) : null}
                  {installed && !running ? (
                    <Button
                      type="button"
                      variant="primary"
                      onClick={handleSenseVoiceStart}
                      disabled={busy}
                    >
                      <Play size={16} />
                      {t("sensevoice.load")}
                    </Button>
                  ) : null}
                  {running ? (
                    <Button
                      type="button"
                      variant="secondary"
                      onClick={handleSenseVoiceStop}
                      disabled={sensevoiceLoading}
                    >
                      <Square size={16} />
                      {t("sensevoice.unload")}
                    </Button>
                  ) : null}
                  {installed && !running ? (
                    <Button
                      type="button"
                      variant="secondary"
                      onClick={handleSenseVoiceRemoveModel}
                      disabled={busy}
                    >
                      <Trash2 size={16} />
                      {t("sensevoice.removeModel")}
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="rounded-full"
                    disabled={busy}
                    onClick={() => void refreshSenseVoiceStatus()}
                    aria-label={t("sensevoice.refresh")}
                  >
                    <RefreshCw size={16} />
                  </Button>
                </div>
              </>
            );
          })()}
        </SettingsCard>
      ) : null}
    </>
  );
}
