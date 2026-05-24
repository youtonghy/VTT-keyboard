import { Info } from "lucide-react";
import { Button, Card, Input } from "@heroui/react";
import { Tooltip } from "./components/Tooltip";
import { PromptTemplateEditor } from "./components/PromptTemplateEditor";
import { SegmentedControl } from "./components/SegmentedControl";
import { Toaster, toast } from "sonner";

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getName, getVersion } from "@tauri-apps/api/app";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { SettingsTabsNav } from "./components/SettingsTabsNav";
import { SettingsCard } from "./components/SettingsCard";
import { HeroCheckboxField, HeroField, HeroInputField } from "./components/HeroField";
import { SpeechSettingsSection } from "./components/settings/SpeechSettingsSection";
import { TextProcessingSettingsSection } from "./components/settings/TextProcessingSettingsSection";
import { RecordingSettingsSection } from "./components/settings/RecordingSettingsSection";
import { MacOSPermissionsSection } from "./components/settings/MacOSPermissionsSection";
import { TagInput } from "./components/TagInput";
import { TitleBar, UpdateStatusControl } from "./components/TitleBar";
import { useAutostart } from "./hooks/useAutostart";
import { usePersistentBoolean } from "./hooks/usePersistentBoolean";
import { useSettings } from "./hooks/useSettings";
import { useUpdater } from "./hooks/useUpdater";
import { useShortcuts } from "./hooks/useShortcuts";
import { useSettingsSync } from "./hooks/useSettingsSync";
import { useSenseVoiceManagement } from "./hooks/useSenseVoiceManagement";
import { useAudioInputDevices, type AudioInputTestResult } from "./hooks/useAudioInputDevices";
import {
  isMacOSPermissionApproved,
  useMacOSPermissions,
  type MacOSPermissionId,
} from "./hooks/useMacOSPermissions";
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
import {
  canStartMicrophoneTest,
  unsupportedMacOSPermissionStatus,
} from "./utils/permissions";

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
  const [historyRetryingId, setHistoryRetryingId] = useState<string | null>(null);
  const [inputTestActive, setInputTestActive] = useState(false);
  const [inputTestResult, setInputTestResult] = useState<AudioInputTestResult | null>(null);
  const supportsSherpaOnnxSenseVoice =
    appInfo?.supportsSherpaOnnxSenseVoice ?? true;
  const isMacOS = appInfo?.platform
    ? appInfo.platform === "macos"
    : navigator.userAgent.includes("Mac OS X");
  const showCustomTitleBar = !isMacOS;
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
  const isRecordingSectionActive = activeSection === "recording";
  const isPermissionsSectionActive = activeSection === "permissions";
  const {
    permissions: macOSPermissions,
    loading: macOSPermissionsLoading,
    error: macOSPermissionsError,
    refreshPermissions,
    requestPermission,
  } = useMacOSPermissions(isMacOS);
  const {
    devices: audioInputDevices,
    error: audioInputDevicesError,
    loading: audioInputDevicesLoading,
    permissionStatus: microphonePermissionStatus,
    refreshDevices,
    testDevice,
  } = useAudioInputDevices(isRecordingSectionActive);

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

  const ensureMicrophonePermission = useCallback(async () => {
    if (!isMacOS) {
      return true;
    }

    const nextPermissions = await refreshPermissions();
    if (isMacOSPermissionApproved(nextPermissions.microphone.status)) {
      return true;
    }

    toast.error(
      t("permissions.blocked.microphone", {
        status: t(`permissions.status.${nextPermissions.microphone.status}`, {
          defaultValue: t("permissions.status.unknown"),
        }),
      })
    );
    setActiveSection("permissions");
    return false;
  }, [isMacOS, refreshPermissions, t]);

  const { isCapturing, setIsCapturing } = useShortcuts(
    draft?.shortcut.key,
    onShortcutCaptured,
    ensureMicrophonePermission
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
    const unlisten = listen<TranscriptionHistoryItem>(
      "transcription-history-updated",
      (event) => {
        setHistoryItems((prev) =>
          prev.map((item) => (item.id === event.payload.id ? event.payload : item))
        );
        setSelectedHistoryItem((prev) =>
          prev?.id === event.payload.id ? event.payload : prev
        );
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
    () => {
      const items = [
        { id: "general", label: t("nav.general") },
        { id: "shortcut", label: t("nav.shortcut") },
        { id: "recording", label: t("nav.recording") },
        { id: "speech", label: t("nav.speech") },
        { id: "text", label: t("nav.text") },
        { id: "triggers", label: t("nav.triggers") },
        { id: "history", label: t("nav.history") },
        { id: "about", label: t("nav.about") },
      ];

      if (!isMacOS) {
        return items;
      }

      return [
        items[0],
        items[1],
        { id: "permissions", label: t("nav.permissions") },
        ...items.slice(2),
      ];
    },
    [isMacOS, t]
  );

  useEffect(() => {
    if (!isMacOS && isPermissionsSectionActive) {
      setActiveSection("general");
    }
  }, [isMacOS, isPermissionsSectionActive]);

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

  useEffect(() => {
    if (!inputTestActive || !draft || activeSection !== "recording") {
      return;
    }

    let cancelled = false;
    const inputDeviceName = draft.recording.inputDeviceName ?? "";

    const runTestLoop = async () => {
      try {
        while (!cancelled) {
          const result = await testDevice(inputDeviceName, 180);
          if (cancelled) {
            return;
          }
          setInputTestResult(result);
        }
      } catch (error) {
        if (!cancelled) {
          toast.error(t("recording.testError", { error: toErrorMessage(error) }));
          setInputTestActive(false);
        }
      }
    };

    void runTestLoop();

    return () => {
      cancelled = true;
    };
  }, [activeSection, draft, inputTestActive, t, testDevice]);

  useEffect(() => {
    if (activeSection !== "recording") {
      setInputTestActive(false);
    }
  }, [activeSection]);

  const handleTestAudioInput = () => {
    if (!inputTestActive) {
      void refreshPermissions()
        .then((nextPermissions) => {
          const microphoneStatus =
            nextPermissions.microphone?.status ??
            unsupportedMacOSPermissionStatus.microphone.status;
          if (!canStartMicrophoneTest(isMacOS, microphoneStatus)) {
            toast.error(
              t("permissions.blocked.microphone", {
                status: t(`permissions.status.${microphoneStatus}`, {
                  defaultValue: t("permissions.status.unknown"),
                }),
              })
            );
            setActiveSection("permissions");
            return;
          }

          setInputTestResult(null);
          setInputTestActive(true);
        })
        .catch((error) => {
          toast.error(t("permissions.refreshError", { error: toErrorMessage(error) }));
          setActiveSection("permissions");
        });
      return;
    }

    setInputTestResult(null);
    setInputTestActive(false);
  };

  const handleRefreshPermissions = () => {
    void refreshPermissions().catch((error) => {
      toast.error(t("permissions.refreshError", { error: toErrorMessage(error) }));
    });
  };

  const handleRequestPermission = (permissionId: MacOSPermissionId) => {
    void requestPermission(permissionId)
      .then((nextPermissions) => {
        const item = nextPermissions[permissionId];
        if (isMacOSPermissionApproved(item.status)) {
          toast.success(t("permissions.requestSuccess"));
          return;
        }
        toast.error(
          t("permissions.requestStillMissing", {
            permission: t(`permissions.items.${permissionId}.title`),
          })
        );
      })
      .catch((error) => {
        toast.error(t("permissions.requestError", { error: toErrorMessage(error) }));
      });
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

  const handleRetryHistoryItem = async (item: TranscriptionHistoryItem) => {
    if (historyRetryingId) {
      return;
    }
    setHistoryRetryingId(item.id);
    try {
      const updated = await invoke<TranscriptionHistoryItem>(
        "retry_transcription_history_item",
        { historyId: item.id }
      );
      setHistoryItems((prev) =>
        prev.map((value) => (value.id === updated.id ? updated : value))
      );
      setSelectedHistoryItem((prev) => (prev?.id === updated.id ? updated : prev));
    } catch (error) {
      toast.error(t("history.retryError", { error: toErrorMessage(error) }));
    } finally {
      setHistoryRetryingId(null);
    }
  };

  if (loading || !draft) {
    return (
      <>

        <Toaster position="top-center" expand={false} theme={draft?.appearance?.theme === "dark" ? "dark" : draft?.appearance?.theme === "light" ? "light" : "system"} />

        {showCustomTitleBar ? (
          <TitleBar
            updateStatus={updater.status}
            onInstallUpdate={updater.installUpdate}
            onRetryUpdateCheck={updater.retryUpdateCheck}
            onDismissUpdateError={updater.dismissUpdateError}
          />
        ) : null}
        <main className={`container loading ${showCustomTitleBar ? "has-custom-titlebar" : ""}`}>
          {!showCustomTitleBar ? (
            <UpdateStatusControl
              className="native-update-banner"
              updateStatus={updater.status}
              onInstallUpdate={updater.installUpdate}
              onRetryUpdateCheck={updater.retryUpdateCheck}
              onDismissUpdateError={updater.dismissUpdateError}
            />
          ) : null}
          <p>{t("app.loading")}</p>
        </main>
      </>
    );
  }

  return (
    <>

      <Toaster position="top-center" expand={false} theme={draft?.appearance?.theme === "dark" ? "dark" : draft?.appearance?.theme === "light" ? "light" : "system"} />

      {showCustomTitleBar ? (
        <TitleBar
          updateStatus={updater.status}
          onInstallUpdate={updater.installUpdate}
          onRetryUpdateCheck={updater.retryUpdateCheck}
          onDismissUpdateError={updater.dismissUpdateError}
        />
      ) : null}
      <main className={`container ${showCustomTitleBar ? "has-custom-titlebar" : ""}`}>
        {!showCustomTitleBar ? (
          <UpdateStatusControl
            className="native-update-banner"
            updateStatus={updater.status}
            onInstallUpdate={updater.installUpdate}
            onRetryUpdateCheck={updater.retryUpdateCheck}
            onDismissUpdateError={updater.dismissUpdateError}
          />
        ) : null}
        <header className="settings-hero">
          <div>
            <p className="settings-hero-eyebrow">VTT Keyboard</p>
            <h1>{t("app.title")}</h1>
            <p>{t("app.subtitle")}</p>
          </div>
        </header>
        <SettingsTabsNav
          items={navItems}
          activeKey={activeSection}
          ariaLabel={t("nav.sectionsAria")}
          onActiveKeyChange={setActiveSection}
        />

        <section className="settings-content" aria-live="polite">
          {activeSection === "general" ? (
            <>
            <SettingsCard
              title={t("general.title")}
              description={t("general.description")}
            >
              <HeroField label={t("general.theme")}>
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
                    { value: "dark", label: t("general.themeDark") },
                  ]}
                />
              </HeroField>
              <HeroField label={t("general.language")}>
                <LanguageSwitcher />
              </HeroField>
              <HeroCheckboxField
                label={t("general.privacyMode")}
                isSelected={draft.privacy.enabled}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    privacy: { ...prev.privacy, enabled: value },
                    provider: value ? "sensevoice" : prev.provider,
                    startup: value
                      ? { ...prev.startup, autoCheckUpdates: false }
                      : prev.startup,
                  }))
                }
              >
                <Tooltip content={t("general.privacyModeHint")}>
                  <span className="hint-icon"><Info size={14} /></span>
                </Tooltip>
              </HeroCheckboxField>
              {isMacOS ? (
                <HeroCheckboxField
                  label={t("general.hideDockIcon")}
                  isSelected={draft.startup.hideDockIcon}
                  onChange={(value) =>
                    updateDraft((prev) => ({
                      ...prev,
                      startup: { ...prev.startup, hideDockIcon: value },
                    }))
                  }
                >
                  <Tooltip content={t("general.hideDockIconHint")}>
                    <span className="hint-icon"><Info size={14} /></span>
                  </Tooltip>
                </HeroCheckboxField>
              ) : null}
              <HeroCheckboxField
                label={t("general.launchOnBoot")}
                isSelected={draft.startup.launchOnBoot}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    startup: { ...prev.startup, launchOnBoot: value },
                  }))
                }
              >
                <Tooltip content={t("general.launchOnBootHint")}>
                  <span className="hint-icon"><Info size={14} /></span>
                </Tooltip>
              </HeroCheckboxField>
              <HeroCheckboxField
                label={t("general.autoCheckUpdates")}
                isSelected={draft.startup.autoCheckUpdates}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    startup: { ...prev.startup, autoCheckUpdates: value },
                  }))
                }
              >
                <Tooltip content={t("general.autoCheckUpdatesHint")}>
                  <span className="hint-icon"><Info size={14} /></span>
                </Tooltip>
              </HeroCheckboxField>
              <HeroCheckboxField
                label={t("general.removeNewlines")}
                isSelected={draft.output.removeNewlines}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    output: { ...prev.output, removeNewlines: value },
                  }))
                }
              >
                <Tooltip content={t("general.removeNewlinesHint")}>
                  <span className="hint-icon"><Info size={14} /></span>
                </Tooltip>
              </HeroCheckboxField>
            </SettingsCard>
            <SettingsCard title={t("data.title")} description={t("data.description")}>
              <div className="button-row">
                <Button type="button" variant="secondary" onPress={handleImport}>
                  {t("data.import")}
                </Button>
                <Button type="button" variant="secondary" onPress={handleExport}>
                  {t("data.export")}
                </Button>
              </div>
            </SettingsCard>
            </>
          ) : null}

          {activeSection === "shortcut" ? (
            <SettingsCard
              title={t("shortcut.title")}
              description={t("shortcut.description")}
            >
              <HeroInputField
                label={t("shortcut.key")}
                value={draft.shortcut.key}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    shortcut: { ...prev.shortcut, key: value },
                  }))
                }
              />
              <div className="shortcut-actions">
                <Button
                  type="button"
                  variant="primary"
                  onPress={() => setIsCapturing(true)}
                  isDisabled={isCapturing}
                >
                  {isCapturing ? t("shortcut.capturing") : t("shortcut.capture")}
                </Button>
                <Tooltip content={t("shortcut.captureHint")}>
                  <span className="hint-icon"><Info size={16} /></span>
                </Tooltip>
              </div>
            </SettingsCard>
          ) : null}

          {isMacOS && activeSection === "permissions" ? (
            <SettingsCard
              title={t("permissions.title")}
              description={t("permissions.description")}
            >
              <MacOSPermissionsSection
                microphone={macOSPermissions.microphone}
                accessibility={macOSPermissions.accessibility}
                loading={macOSPermissionsLoading}
                error={macOSPermissionsError}
                t={t}
                onRefresh={handleRefreshPermissions}
                onRequestPermission={handleRequestPermission}
              />
            </SettingsCard>
          ) : null}

          {activeSection === "recording" ? (
            <SettingsCard
              title={t("recording.title")}
              description={t("recording.description")}
            >
              <RecordingSettingsSection
                draft={draft}
                devices={audioInputDevices}
                devicesError={audioInputDevicesError}
                devicesLoading={audioInputDevicesLoading}
                microphonePermissionStatus={microphonePermissionStatus}
                inputTestResult={inputTestResult}
                inputTestActive={inputTestActive}
                t={t}
                onRefreshDevices={refreshDevices}
                onTestInput={handleTestAudioInput}
                updateDraft={updateDraft}
              />
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
                  <Card key={card.id} className="trigger-card">
                    <Card.Header className="trigger-card-header">
                      <Input
                        className="trigger-title-input"
                        value={card.title}
                        onChange={(event) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            title: event.target.value,
                          }))
                        }
                      />
                      <div className="trigger-card-actions">
                        <Button
                          type="button"
                          variant="secondary"
                          onPress={() => moveTrigger(index, index - 1)}
                          isDisabled={index === 0}
                        >
                          {t("triggers.moveUp")}
                        </Button>
                        <Button
                          type="button"
                          variant="secondary"
                          onPress={() => moveTrigger(index, index + 1)}
                          isDisabled={index === draft.triggers.length - 1}
                        >
                          {t("triggers.moveDown")}
                        </Button>
                        <Button
                          type="button"
                          variant="danger"
                          onPress={() => removeTrigger(card.id)}
                          isDisabled={card.locked}
                        >
                          {t("triggers.remove")}
                        </Button>
                      </div>
                    </Card.Header>
                    <Card.Content className="trigger-card-body">
                      <HeroCheckboxField
                        label={t("triggers.enabled")}
                        isSelected={card.enabled}
                        onChange={(value) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            enabled: value,
                          }))
                        }
                      />
                      <HeroCheckboxField
                        label={t("triggers.autoApply")}
                        isSelected={card.autoApply}
                        onChange={(value) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            autoApply: value,
                          }))
                        }
                      />
                      <HeroInputField
                        label={t("triggers.keyword")}
                        value={card.keyword}
                        onChange={(value) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            keyword: value,
                          }))
                        }
                      />
                      <HeroField label={t("triggers.variables")}>
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
                      </HeroField>
                      <HeroField label={t("triggers.promptTemplate")}>
                        <PromptTemplateEditor
                          value={card.promptTemplate}
                          onChange={(value) =>
                            updateTrigger(card.id, (prev) => ({
                              ...prev,
                              promptTemplate: value,
                            }))
                          }
                        />
                      </HeroField>
                    </Card.Content>
                  </Card>
                ))}
              </div>
              <Button type="button" variant="secondary" onPress={addTrigger}>
                {t("triggers.add")}
              </Button>
            </SettingsCard>
          ) : null}

          {activeSection === "history" ? (
            <SettingsCard
              title={t("history.title")}
              description={t("history.description")}
            >
              <HeroCheckboxField
                label={t("history.enabled")}
                isSelected={draft.history.enabled}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    history: { ...prev.history, enabled: value },
                  }))
                }
              />

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
                      <Button
                        key={item.id}
                        type="button"
                        className={`history-item ${isFailed ? "failed" : ""} ${isKeywordTriggered ? "triggered" : ""}`}
                        variant="secondary"
                        onPress={() => setSelectedHistoryItem(item)}
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
                      </Button>
                    );
                  })}
                </div>
              )}

              <div className="history-actions">
                <Button
                  type="button"
                  variant="danger"
                  onPress={handleClearHistory}
                  isDisabled={historyItems.length === 0}
                >
                  {t("history.clear")}
                </Button>
              </div>
            </SettingsCard>
          ) : null}

          {activeSection === "about" ? (
            appInfo ? (
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
            ) : null
          ) : null}
        </section>
      </main>
      <HistoryDetailDialog
        item={selectedHistoryItem}
        isRetrying={Boolean(selectedHistoryItem && historyRetryingId === selectedHistoryItem.id)}
        onClose={() => setSelectedHistoryItem(null)}
        onRetry={handleRetryHistoryItem}
      />
    </>
  );
}

export default App;
