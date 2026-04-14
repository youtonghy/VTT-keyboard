import { Info } from "lucide-react";
import { Tooltip } from "./components/Tooltip";
import { PromptTemplateEditor } from "./components/PromptTemplateEditor";
import { NumberWheelInput } from "./components/NumberWheelInput";
import { SegmentedControl } from "./components/SegmentedControl";
import { Toaster, toast } from "sonner";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getName, getVersion } from "@tauri-apps/api/app";
import { Sidebar } from "./components/Sidebar";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { SettingsCard } from "./components/SettingsCard";
import { SpeechSettingsSection } from "./components/settings/SpeechSettingsSection";
import { TextProcessingSettingsSection } from "./components/settings/TextProcessingSettingsSection";
import { TagInput } from "./components/TagInput";
import { TitleBar } from "./components/TitleBar";
import { useAutostart } from "./hooks/useAutostart";
import { usePersistentBoolean } from "./hooks/usePersistentBoolean";
import { useSettings } from "./hooks/useSettings";
import { useUpdater } from "./hooks/useUpdater";
import { useShortcuts } from "./hooks/useShortcuts";
import { useSettingsSync } from "./hooks/useSettingsSync";
import { useSenseVoiceManagement } from "./hooks/useSenseVoiceManagement";
import { HistoryDetailDialog } from "./components/HistoryDetailDialog";
import type { TranscriptionHistoryItem } from "./types/history";
import type { Settings } from "./types/settings";

import { parseList, toErrorMessage } from "./utils";
import {
  normalizeLocalModel,
  normalizeSenseVoiceLanguage,
  normalizeSenseVoiceDevice,
  isCudaOnlyLocalModel,
  getDefaultModelId,
  getQwenVariantByModelId,
  formatBytes,
  SHERPA_LANGUAGE_OPTIONS,
  QWEN3_ASR_MODEL_VARIANTS,
} from "./utils/sensevoice";

const MAX_HISTORY_ITEMS = 200;
const HISTORY_PREVIEW_MAX_CHARS = 50;

const createId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;

interface AppInfoPayload {
  buildDate: string;
  platform: string;
  arch: string;
  supportsSherpaOnnxSenseVoice: boolean;
}

const formatHistoryTime = (timestampMs: number) => {
  if (!Number.isFinite(timestampMs) || timestampMs <= 0) {
    return "--:-- --/--";
  }
  const value = new Date(timestampMs);
  if (Number.isNaN(value.getTime())) {
    return "--:-- --/--";
  }
  const hour = String(value.getHours()).padStart(2, "0");
  const minute = String(value.getMinutes()).padStart(2, "0");
  return `${hour}:${minute} ${value.getDate()}/${value.getMonth() + 1}`;
};

const buildHistoryPreview = (text: string, maxChars: number, ellipsis: string) => {
  const chars = Array.from(text);
  if (chars.length <= maxChars) {
    return {
      preview: text,
      truncated: false,
    };
  }
  return {
    preview: `${chars.slice(0, maxChars).join("")}${ellipsis}`,
    truncated: true,
  };
};

function App() {
  const { t, i18n } = useTranslation();
  const { settings, setSettings, loading, saveSettings } = useSettings();
  const { syncAutostart } = useAutostart();
  const [activeSection, setActiveSection] = useState("general");
  const [sidebarCollapsed, setSidebarCollapsed] = usePersistentBoolean(
    "vtt.sidebar.collapsed",
    false
  );
  const [sensevoiceLogsExpanded, setSensevoiceLogsExpanded] = usePersistentBoolean(
    "vtt.sensevoice.logs.expanded",
    false
  );
  const updater = useUpdater();
  const [appInfo, setAppInfo] = useState<
    ({ name: string; version: string } & AppInfoPayload) | null
  >(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyItems, setHistoryItems] = useState<TranscriptionHistoryItem[]>([]);
  const [selectedHistoryItem, setSelectedHistoryItem] =
    useState<TranscriptionHistoryItem | null>(null);
  const supportsSherpaOnnxSenseVoice =
    appInfo?.supportsSherpaOnnxSenseVoice ?? true;
  const sherpaFallbackActive =
    !supportsSherpaOnnxSenseVoice &&
    normalizeLocalModel(settings?.sensevoice.localModel) === "sherpa-onnx-sensevoice";

  const { draft, updateDraft, handleImport, handleExport } = useSettingsSync({
    settings,
    setSettings,
    saveSettings,
    syncAutostart,
    supportsSherpaOnnxSenseVoice,
  });

  const isSenseVoiceActive = activeSection === "speech" && draft?.provider === "sensevoice";

  const {
    sensevoiceStatus,
    sensevoiceProgress,
    sensevoiceLogLines,
    sensevoiceLoading,
    handleSenseVoicePrepare,
    handleSenseVoiceStart,
    handleSenseVoiceStop,
    handleUpdateRuntime,
  } = useSenseVoiceManagement({
    isSenseVoiceActive,
    draft,
    supportsSherpaOnnxSenseVoice,
  });

  const onShortcutCaptured = useCallback(
    (key: string) => {
      updateDraft((prev) => ({
        ...prev,
        shortcut: { ...prev.shortcut, key },
      }));
    },
    [updateDraft]
  );

  const { isCapturing, setIsCapturing } = useShortcuts(
    draft?.shortcut.key,
    onShortcutCaptured
  );

  const loadHistory = useCallback(async () => {
    setHistoryLoading(true);
    try {
      const items = await invoke<TranscriptionHistoryItem[]>("get_transcription_history");
      setHistoryItems(items.slice(0, MAX_HISTORY_ITEMS));
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  useEffect(() => {
    const fetchAppInfo = async () => {
      const [name, version, info] = await Promise.all([
        getName(),
        getVersion(),
        invoke<AppInfoPayload>("get_app_info"),
      ]);
      setAppInfo({ name, version, ...info });
    };
    void fetchAppInfo();
  }, []);

  useEffect(() => {
    if (!draft) {
      return;
    }
    const root = document.documentElement;
    if (draft.appearance.theme === "system") {
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      const applyTheme = () => {
        root.setAttribute("data-theme", media.matches ? "dark" : "light");
      };
      applyTheme();
      media.addEventListener("change", applyTheme);
      return () => media.removeEventListener("change", applyTheme);
    }
    root.setAttribute("data-theme", draft.appearance.theme);
  }, [draft?.appearance.theme]);

  useEffect(() => {
    void invoke("set_tray_menu", {
      labels: {
        showSettings: t("tray.showSettings"),
        quit: t("tray.quit"),
      },
    });
  }, [i18n.language, t]);

  useEffect(() => {
    const unlisten = listen<TranscriptionHistoryItem>(
      "transcription-history-appended",
      (event) => {
        setHistoryItems((prev) => {
          if (prev.some((item) => item.id === event.payload.id)) {
            return prev;
          }
          return [event.payload, ...prev].slice(0, MAX_HISTORY_ITEMS);
        });
      }
    );
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  useEffect(() => {
    if (activeSection !== "history") {
      setSelectedHistoryItem(null);
      return;
    }
    void loadHistory().catch((error) => {
      toast.error(t("history.loadError", { error: toErrorMessage(error) }));
    });
  }, [activeSection, loadHistory, t]);

  // 窗口重新获得焦点时刷新历史，补偿窗口隐藏期间可能丢失的事件
  useEffect(() => {
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused && activeSection === "history") {
        void loadHistory().catch((error) => {
          toast.error(t("history.loadError", { error: toErrorMessage(error) }));
        });
      }
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [activeSection, loadHistory, t]);

  const navItems = useMemo(
    () => [
      { id: "general", label: t("nav.general") },
      { id: "shortcut", label: t("nav.shortcut") },
      { id: "recording", label: t("nav.recording") },
      { id: "speech", label: t("nav.speech") },
      { id: "text", label: t("nav.text") },
      { id: "triggers", label: t("nav.triggers") },
      { id: "history", label: t("nav.history") },
      { id: "about", label: t("nav.about") },
    ],
    [t]
  );

  const createTriggerCard = () => ({
    id: createId(),
    title: t("triggers.newTitle"),
    enabled: true,
    autoApply: false,
    locked: false,
    keyword: t("triggers.defaultKeyword"),
    promptTemplate: t("triggers.defaultTemplate"),
    variables: parseList(t("triggers.defaultVariables")),
  });

  const updateTrigger = (
    id: string,
    updater: (card: Settings["triggers"][number]) => Settings["triggers"][number]
  ) => {
    updateDraft((prev) => ({
      ...prev,
      triggers: prev.triggers.map((card) => (card.id === id ? updater(card) : card)),
    }));
  };

  const moveTrigger = (from: number, to: number) => {
    updateDraft((prev) => {
      const next = [...prev.triggers];
      const [item] = next.splice(from, 1);
      next.splice(to, 0, item);
      return { ...prev, triggers: next };
    });
  };

  const removeTrigger = (id: string) => {
    updateDraft((prev) => ({
      ...prev,
      triggers: prev.triggers.filter((card) => card.id !== id || card.locked),
    }));
  };

  const addTrigger = () => {
    updateDraft((prev) => ({
      ...prev,
      triggers: [...prev.triggers, createTriggerCard()],
    }));
  };

  const handleClearHistory = async () => {
    const confirmed = window.confirm(t("history.clearConfirm"));
    if (!confirmed) {
      return;
    }
    try {
      await invoke("clear_transcription_history");
      setHistoryItems([]);
      setSelectedHistoryItem(null);
      toast.success(t("history.clearSuccess"));
    } catch (error) {
      toast.error(t("history.clearError", { error: toErrorMessage(error) }));
    }
  };

  if (loading || !draft) {
    return (
      <>

        <Toaster position="top-center" expand={false} theme={draft?.appearance?.theme === "dark" ? "dark" : draft?.appearance?.theme === "light" ? "light" : "system"} />

        <TitleBar
          updateStatus={updater.status}
          onInstallUpdate={updater.installUpdate}
          onRetryUpdateCheck={updater.retryUpdateCheck}
          onDismissUpdateError={updater.dismissUpdateError}
        />
        <main className="container loading">
          <p>{t("app.loading")}</p>
        </main>
      </>
    );
  }

  return (
    <>

      <Toaster position="top-center" expand={false} theme={draft?.appearance?.theme === "dark" ? "dark" : draft?.appearance?.theme === "light" ? "light" : "system"} />

      <TitleBar
        updateStatus={updater.status}
        onInstallUpdate={updater.installUpdate}
        onRetryUpdateCheck={updater.retryUpdateCheck}
        onDismissUpdateError={updater.dismissUpdateError}
      />
      <main className="container">
        <div className="settings-layout">
          <Sidebar
            items={navItems}
            activeId={activeSection}
            collapsed={sidebarCollapsed}
            onToggleCollapsed={() => setSidebarCollapsed((prev) => !prev)}
            onSelect={setActiveSection}
          />
          <section className="settings-content">
            {activeSection === "general" ? (
              <>
                <SettingsCard
                  title={t("general.title")}
                  description={t("general.description")}
                >
                  <label className="field">
                    <span>{t("general.theme")}</span>
                    <SegmentedControl
  value={draft.appearance.theme}
  onChange={(value) =>
    updateDraft((prev) => ({
      ...prev,
      appearance: { ...prev.appearance, theme: value },
    }))
  }
  options={[
    { value: "system", label: t("general.themeSystem") },
    { value: "light", label: t("general.themeLight") },
    { value: "dark", label: t("general.themeDark") }
  ]}
/>
                  </label>
                  <div className="field">
                    <span>{t("general.language")}</span>
                    <LanguageSwitcher />
                  </div>
                  <label className="field checkbox">
                    <input
                      type="checkbox"
                      checked={draft.startup.launchOnBoot}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          startup: {
                            ...prev.startup,
                            launchOnBoot: event.target.checked,
                          },
                        }))
                      }
                    />
                    <span>{t("general.launchOnBoot")}</span>
  <Tooltip content={t("general.launchOnBootHint")}>
    <span className="flex items-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors"><Info size={14} /></span>
  </Tooltip>
</label>
                  <label className="field checkbox">
                    <input
                      type="checkbox"
                      checked={draft.startup.autoCheckUpdates}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          startup: {
                            ...prev.startup,
                            autoCheckUpdates: event.target.checked,
                          },
                        }))
                      }
                    />
                    <span>{t("general.autoCheckUpdates")}</span>
  <Tooltip content={t("general.autoCheckUpdatesHint")}>
    <span className="flex items-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors"><Info size={14} /></span>
  </Tooltip>
</label>
                  <label className="field checkbox">
                    <input
                      type="checkbox"
                      checked={draft.startup.autoInstallUpdatesOnQuit}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          startup: {
                            ...prev.startup,
                            autoInstallUpdatesOnQuit: event.target.checked,
                          },
                        }))
                      }
                    />
                    <span>{t("general.autoInstallUpdatesOnQuit")}</span>
  <Tooltip content={t("general.autoInstallUpdatesOnQuitHint")}>
    <span className="flex items-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors"><Info size={14} /></span>
  </Tooltip>
</label>
                  <label className="field checkbox">
                    <input
                      type="checkbox"
                      checked={draft.output.removeNewlines}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          output: {
                            ...prev.output,
                            removeNewlines: event.target.checked,
                          },
                        }))
                      }
                    />
                    <span>{t("general.removeNewlines")}</span>
  <Tooltip content={t("general.removeNewlinesHint")}>
    <span className="flex items-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors"><Info size={14} /></span>
  </Tooltip>
</label>
                </SettingsCard>
                <SettingsCard title={t("data.title")} description={t("data.description")}>
                  <div className="button-row">
                    <button type="button" onClick={handleImport}>
                      {t("data.import")}
                    </button>
                    <button type="button" onClick={handleExport}>
                      {t("data.export")}
                    </button>
                  </div>
                </SettingsCard>
              </>
            ) : null}

            {activeSection === "shortcut" ? (
              <SettingsCard
                title={t("shortcut.title")}
                description={t("shortcut.description")}
              >
                <label className="field">
                  <span>{t("shortcut.key")}</span>
                  <input
                    value={draft.shortcut.key}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        shortcut: { ...prev.shortcut, key: event.target.value },
                      }))
                    }
                  />
                </label>
                <div className="shortcut-actions">
  <button
    type="button"
    onClick={() => setIsCapturing(true)}
    disabled={isCapturing}
  >
    {isCapturing ? t("shortcut.capturing") : t("shortcut.capture")}
  </button>
  <Tooltip content={t("shortcut.captureHint")}>
    <span className="flex items-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors"><Info size={16} /></span>
  </Tooltip>
</div>
              </SettingsCard>
            ) : null}

            {activeSection === "recording" ? (
              <SettingsCard
                title={t("recording.title")}
                description={t("recording.description")}
              >
                <label className="field">
                  <span>{t("recording.segmentSeconds")}</span>
                  <NumberWheelInput
  min={10}
  value={draft.recording.segmentSeconds}
  onChange={(value) =>
    updateDraft((prev) => ({
      ...prev,
      recording: {
        ...prev.recording,
        segmentSeconds: value,
      },
    }))
  }
/>
                </label>
              </SettingsCard>
            ) : null}

            {activeSection === "speech" ? (
              <SpeechSettingsSection
                draft={draft}
                t={t}
                updateDraft={updateDraft}
                supportsSherpaOnnxSenseVoice={supportsSherpaOnnxSenseVoice}
                sherpaFallbackActive={sherpaFallbackActive}
                sensevoiceStatus={sensevoiceStatus}
                sensevoiceProgress={sensevoiceProgress}
                sensevoiceLogLines={sensevoiceLogLines}
                sensevoiceLogsExpanded={sensevoiceLogsExpanded}
                setSensevoiceLogsExpanded={setSensevoiceLogsExpanded}
                sensevoiceLoading={sensevoiceLoading}
                handleSenseVoicePrepare={handleSenseVoicePrepare}
                handleSenseVoiceStart={handleSenseVoiceStart}
                handleSenseVoiceStop={handleSenseVoiceStop}
                handleUpdateRuntime={handleUpdateRuntime}
                normalizeLocalModel={normalizeLocalModel}
                normalizeSenseVoiceLanguage={normalizeSenseVoiceLanguage}
                normalizeSenseVoiceDevice={normalizeSenseVoiceDevice}
                isCudaOnlyLocalModel={isCudaOnlyLocalModel}
                getDefaultModelId={getDefaultModelId}
                getQwenVariantByModelId={getQwenVariantByModelId}
                formatBytes={formatBytes}
                sherpaLanguageOptions={SHERPA_LANGUAGE_OPTIONS.map((option) => ({
                  value: option.value,
                  label: t(option.labelKey),
                }))}
                qwenVariantOptions={QWEN3_ASR_MODEL_VARIANTS.map((option) => ({
                  value: option.value,
                  label: t(option.labelKey),
                }))}
              />
            ) : null}

            {activeSection === "text" ? (
              <TextProcessingSettingsSection
                draft={draft}
                t={t}
                updateDraft={updateDraft}
              />
            ) : null}

            {activeSection === "triggers" ? (
              <SettingsCard
                title={t("triggers.title")}
                description={t("triggers.description")}
              >
                <div className="trigger-list">
                  {draft.triggers.map((card, index) => (
                    <div key={card.id} className="trigger-card">
                      <div className="trigger-card-header">
                        <input
                          value={card.title}
                          onChange={(event) =>
                            updateTrigger(card.id, (prev) => ({
                              ...prev,
                              title: event.target.value,
                            }))
                          }
                        />
                        <div className="trigger-card-actions">
                          <button
                            type="button"
                            onClick={() => moveTrigger(index, index - 1)}
                            disabled={index === 0}
                          >
                            {t("triggers.moveUp")}
                          </button>
                          <button
                            type="button"
                            onClick={() => moveTrigger(index, index + 1)}
                            disabled={index === draft.triggers.length - 1}
                          >
                            {t("triggers.moveDown")}
                          </button>
                          <button
                            type="button"
                            onClick={() => removeTrigger(card.id)}
                            disabled={card.locked}
                          >
                            {t("triggers.remove")}
                          </button>
                        </div>
                      </div>
                      <div className="trigger-card-body">
                        <label className="field checkbox">
                          <input
                            type="checkbox"
                            checked={card.enabled}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                enabled: event.target.checked,
                              }))
                            }
                          />
                          <span>{t("triggers.enabled")}</span>
                        </label>
                        <label className="field checkbox">
                          <input
                            type="checkbox"
                            checked={card.autoApply}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                autoApply: event.target.checked,
                              }))
                            }
                          />
                          <span>{t("triggers.autoApply")}</span>
                        </label>
                        <label className="field">
                          <span>{t("triggers.keyword")}</span>
                          <input
                            value={card.keyword}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                keyword: event.target.value,
                              }))
                            }
                          />
                        </label>
                        <label className="field">
                          <span>{t("triggers.variables")}</span>
                          <TagInput
                            values={card.variables}
                            placeholder={t("triggers.variablesPlaceholder")}
                            onCommit={(nextValues) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                variables: nextValues,
                              }))
                            }
                          />
                        </label>
                        <label className="field">
                          <span>{t("triggers.promptTemplate")}</span>
                          <PromptTemplateEditor
  value={card.promptTemplate}
  onChange={(value) =>
    updateTrigger(card.id, (prev) => ({
      ...prev,
      promptTemplate: value,
    }))
  }
/>
                        </label>
                      </div>
                    </div>
                  ))}
                </div>
                <button type="button" className="secondary" onClick={addTrigger}>
                  {t("triggers.add")}
                </button>
              </SettingsCard>
            ) : null}

            {activeSection === "history" ? (
              <SettingsCard
                title={t("history.title")}
                description={t("history.description")}
              >
                <label className="field checkbox">
                  <input
                    type="checkbox"
                    checked={draft.history.enabled}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        history: {
                          ...prev.history,
                          enabled: event.target.checked,
                        },
                      }))
                    }
                  />
                  <span>{t("history.enabled")}</span>
                </label>

                {historyLoading ? (
                  <div className="history-empty">{t("history.loading")}</div>
                ) : historyItems.length === 0 ? (
                  <div className="history-empty">{t("history.empty")}</div>
                ) : (
                  <div className="history-list">
                    {historyItems.map((item) => {
                      const isFailed = item.status === "failed";
                      const isKeywordTriggered = !isFailed && item.triggeredByKeyword;
                      const mainText = isFailed
                        ? t("history.failed")
                        : isKeywordTriggered
                          ? item.finalText || t("history.emptyText")
                          : item.transcriptionText || t("history.emptyText");
                      const { preview, truncated } = buildHistoryPreview(
                        mainText,
                        HISTORY_PREVIEW_MAX_CHARS,
                        t("history.previewEllipsis")
                      );

                      return (
                        <button
                          key={item.id}
                          type="button"
                          className={`history-item ${isFailed ? "failed" : ""} ${isKeywordTriggered ? "triggered" : ""}`}
                          onClick={() => setSelectedHistoryItem(item)}
                        >
                          <span
                            className="history-item-content"
                            title={truncated ? mainText : undefined}
                          >
                            {preview}
                          </span>
                          <span className="history-item-time">
                            {formatHistoryTime(item.timestampMs)}
                          </span>
                        </button>
                      );
                    })}
                  </div>
                )}

                <div className="history-actions">
                  <button
                    type="button"
                    className="danger"
                    onClick={handleClearHistory}
                    disabled={historyItems.length === 0}
                  >
                    {t("history.clear")}
                  </button>
                </div>
              </SettingsCard>
            ) : null}

            {activeSection === "about" && appInfo ? (
              <SettingsCard
                title={t("about.title")}
                description={t("about.description")}
              >
                <div className="field">
                  <span>{t("about.appName")}</span>
                  <span>{appInfo.name}</span>
                </div>
                <div className="field">
                  <span>{t("about.version")}</span>
                  <span>{appInfo.version}</span>
                </div>
                <div className="field">
                  <span>{t("about.buildDate")}</span>
                  <span>{appInfo.buildDate}</span>
                </div>
                <div className="field">
                  <span>{t("about.author")}</span>
                  <span>youtonghy</span>
                </div>
                <div className="field">
                  <span>{t("about.website")}</span>
                  <a
                    href="https://vtt.tokisantike.net/"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    https://vtt.tokisantike.net/
                  </a>
                </div>
              </SettingsCard>
            ) : null}
          </section>
        </div>
        
      </main>
      <HistoryDetailDialog
        item={selectedHistoryItem}
        onClose={() => setSelectedHistoryItem(null)}
      />
    </>
  );
}

export default App;
