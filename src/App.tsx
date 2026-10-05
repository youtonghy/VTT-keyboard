import { ArrowDown, ArrowUp, PanelLeft, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/motion/button/base";
import {
  AnimatedSidebarInset,
  AnimatedSidebarProvider,
  AnimatedSidebarTrigger,
} from "@/components/motion/animated-sidebar";
import { cn } from "@/lib/utils";
import { Tooltip } from "./components/Tooltip";
import { HintIcon } from "./components/HintIcon";
import { AppSidebar } from "./components/AppSidebar";
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
import { SettingsCard } from "./components/SettingsCard";
import { Field, InputField, ToggleField, inputClassName } from "./components/FormFields";
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
  const isMacOS = appInfo?.platform
    ? appInfo.platform === "macos"
    : navigator.userAgent.includes("Mac OS X");
  const showCustomTitleBar = !isMacOS;

  const { draft, updateDraft, handleImport, handleExport } = useSettingsSync({
    settings,
    setSettings,
    saveSettings,
    syncAutostart,
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
    handleSenseVoiceRemoveModel,
    handleSenseVoiceConfigSave,
    refreshSenseVoiceStatus,
  } = useSenseVoiceManagement({
    isSenseVoiceActive,
    draft,
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

  const toasterTheme =
    draft?.appearance?.theme === "dark"
      ? "dark"
      : draft?.appearance?.theme === "light"
        ? "light"
        : "system";

  const titleBar = showCustomTitleBar ? (
    <TitleBar
      updateStatus={updater.status}
      onInstallUpdate={updater.installUpdate}
      onRetryUpdateCheck={updater.retryUpdateCheck}
      onDismissUpdateError={updater.dismissUpdateError}
    />
  ) : null;

  const nativeUpdateStatus = !showCustomTitleBar ? (
    <UpdateStatusControl
      className="native-update-banner"
      updateStatus={updater.status}
      onInstallUpdate={updater.installUpdate}
      onRetryUpdateCheck={updater.retryUpdateCheck}
      onDismissUpdateError={updater.dismissUpdateError}
    />
  ) : null;

  if (loading || !draft) {
    return (
      <>
        <Toaster position="top-center" expand={false} theme={toasterTheme} />
        {titleBar}
        <main
          className={cn(
            "flex h-full flex-col items-center justify-center gap-3 text-muted-foreground text-sm",
            showCustomTitleBar && "pt-10",
          )}
        >
          {nativeUpdateStatus}
          <p>{t("app.loading")}</p>
        </main>
      </>
    );
  }

  const activeLabel = navItems.find((item) => item.id === activeSection)?.label ?? "";

  return (
    <>
      <Toaster position="top-center" expand={false} theme={toasterTheme} />
      {titleBar}
      <AnimatedSidebarProvider
        className={cn("h-full min-h-0 overflow-hidden", showCustomTitleBar && "pt-10")}
      >
        <AppSidebar
          items={navItems}
          activeId={activeSection}
          onSelect={setActiveSection}
          version={appInfo?.version}
        />
        <AnimatedSidebarInset className="min-h-0">
          <header className="flex h-14 shrink-0 items-center gap-3 border-b border-border px-4">
            <AnimatedSidebarTrigger className="text-muted-foreground transition-colors hover:bg-muted hover:text-foreground">
              <PanelLeft aria-hidden="true" className="size-4" />
            </AnimatedSidebarTrigger>
            <div className="h-5 w-px bg-border" />
            <h1 className="min-w-0 flex-1 truncate font-medium text-foreground text-sm">
              {activeLabel}
            </h1>
            {nativeUpdateStatus}
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto">
        <section
          className="mx-auto grid w-full max-w-3xl gap-5 px-5 py-6 sm:px-8"
          aria-live="polite"
        >
          {activeSection === "general" ? (
            <>
            <SettingsCard
              title={t("general.title")}
              description={t("general.description")}
            >
              <Field label={t("general.theme")}>
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
              </Field>
              <Field label={t("general.language")}>
                <LanguageSwitcher />
              </Field>
              <ToggleField
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
                <HintIcon content={t("general.privacyModeHint")} />
              </ToggleField>
              {isMacOS ? (
                <ToggleField
                  label={t("general.hideDockIcon")}
                  isSelected={draft.startup.hideDockIcon}
                  onChange={(value) =>
                    updateDraft((prev) => ({
                      ...prev,
                      startup: { ...prev.startup, hideDockIcon: value },
                    }))
                  }
                >
                  <HintIcon content={t("general.hideDockIconHint")} />
                </ToggleField>
              ) : null}
              <ToggleField
                label={t("general.launchOnBoot")}
                isSelected={draft.startup.launchOnBoot}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    startup: { ...prev.startup, launchOnBoot: value },
                  }))
                }
              >
                <HintIcon content={t("general.launchOnBootHint")} />
              </ToggleField>
              <ToggleField
                label={t("general.autoCheckUpdates")}
                isSelected={draft.startup.autoCheckUpdates}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    startup: { ...prev.startup, autoCheckUpdates: value },
                  }))
                }
              >
                <HintIcon content={t("general.autoCheckUpdatesHint")} />
              </ToggleField>
              <ToggleField
                label={t("general.removeNewlines")}
                isSelected={draft.output.removeNewlines}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    output: { ...prev.output, removeNewlines: value },
                  }))
                }
              >
                <HintIcon content={t("general.removeNewlinesHint")} />
              </ToggleField>
            </SettingsCard>
            <SettingsCard title={t("data.title")} description={t("data.description")}>
              <div className="flex flex-wrap gap-2">
                <Button type="button" variant="secondary" onClick={handleImport}>
                  {t("data.import")}
                </Button>
                <Button type="button" variant="secondary" onClick={handleExport}>
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
              <InputField
                label={t("shortcut.key")}
                value={draft.shortcut.key}
                onChange={(value) =>
                  updateDraft((prev) => ({
                    ...prev,
                    shortcut: { ...prev.shortcut, key: value },
                  }))
                }
              />
              <div className="flex items-center gap-2">
                <Button
                  type="button"
                  variant="primary"
                  onClick={() => setIsCapturing(true)}
                  disabled={isCapturing}
                >
                  {isCapturing ? t("shortcut.capturing") : t("shortcut.capture")}
                </Button>
                <HintIcon content={t("shortcut.captureHint")} size={16} />
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
              sensevoiceStatus={sensevoiceStatus}
              sensevoiceProgress={sensevoiceProgress}
              sensevoiceLogLines={sensevoiceLogLines}
              sensevoiceLogsExpanded={sensevoiceLogsExpanded}
              setSensevoiceLogsExpanded={setSensevoiceLogsExpanded}
              sensevoiceLoading={sensevoiceLoading}
              handleSenseVoicePrepare={handleSenseVoicePrepare}
              handleSenseVoiceStart={handleSenseVoiceStart}
              handleSenseVoiceStop={handleSenseVoiceStop}
              handleSenseVoiceRemoveModel={handleSenseVoiceRemoveModel}
              handleSenseVoiceConfigSave={handleSenseVoiceConfigSave}
              refreshSenseVoiceStatus={refreshSenseVoiceStatus}
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
              <div className="grid gap-4">
                {draft.triggers.map((card, index) => (
                  <div key={card.id} className="grid gap-4 rounded-2xl border border-border bg-muted/30 p-4">
                    <div className="flex flex-wrap items-center gap-2">
                      <input
                        className={cn(inputClassName, "min-w-40 flex-1 font-medium")}
                        aria-label={t("triggers.title")}
                        value={card.title}
                        onChange={(event) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            title: event.target.value,
                          }))
                        }
                      />
                      <div className="flex items-center gap-1">
                        <Tooltip content={t("triggers.moveUp")}>
                          <Button
                            variant="ghost"
                            size="icon"
                            className="rounded-full"
                            aria-label={t("triggers.moveUp")}
                            onClick={() => moveTrigger(index, index - 1)}
                            disabled={index === 0}
                          >
                            <ArrowUp size={16} />
                          </Button>
                        </Tooltip>
                        <Tooltip content={t("triggers.moveDown")}>
                          <Button
                            variant="ghost"
                            size="icon"
                            className="rounded-full"
                            aria-label={t("triggers.moveDown")}
                            onClick={() => moveTrigger(index, index + 1)}
                            disabled={index === draft.triggers.length - 1}
                          >
                            <ArrowDown size={16} />
                          </Button>
                        </Tooltip>
                        <Tooltip content={t("triggers.remove")}>
                          <Button
                            variant="danger"
                            size="icon"
                            className="rounded-full"
                            aria-label={t("triggers.remove")}
                            onClick={() => removeTrigger(card.id)}
                            disabled={card.locked}
                          >
                            <Trash2 size={16} />
                          </Button>
                        </Tooltip>
                      </div>
                    </div>
                    <div className="grid gap-4">
                      <ToggleField
                        label={t("triggers.enabled")}
                        isSelected={card.enabled}
                        onChange={(value) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            enabled: value,
                          }))
                        }
                      />
                      <ToggleField
                        label={t("triggers.autoApply")}
                        isSelected={card.autoApply}
                        onChange={(value) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            autoApply: value,
                          }))
                        }
                      />
                      <InputField
                        label={t("triggers.keyword")}
                        value={card.keyword}
                        onChange={(value) =>
                          updateTrigger(card.id, (prev) => ({
                            ...prev,
                            keyword: value,
                          }))
                        }
                      />
                      <Field label={t("triggers.variables")}>
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
                      </Field>
                      <Field label={t("triggers.promptTemplate")}>
                        <PromptTemplateEditor
                          value={card.promptTemplate}
                          onChange={(value) =>
                            updateTrigger(card.id, (prev) => ({
                              ...prev,
                              promptTemplate: value,
                            }))
                          }
                        />
                      </Field>
                    </div>
                  </div>
                ))}
              </div>
              <div>
                <Button type="button" variant="secondary" onClick={addTrigger}>
                  <Plus size={16} />
                  {t("triggers.add")}
                </Button>
              </div>
            </SettingsCard>
          ) : null}

          {activeSection === "history" ? (
            <SettingsCard
              title={t("history.title")}
              description={t("history.description")}
            >
              <ToggleField
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
                <div className="rounded-2xl border border-dashed border-border py-10 text-center text-muted-foreground text-sm">{t("history.loading")}</div>
              ) : historyItems.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-border py-10 text-center text-muted-foreground text-sm">{t("history.empty")}</div>
              ) : (
                <div className="divide-y divide-border overflow-hidden rounded-2xl border border-border">
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
                        className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-muted/60 focus-visible:bg-muted/60 focus-visible:outline-none"
                        onClick={() => setSelectedHistoryItem(item)}
                      >
                        <span
                          aria-hidden="true"
                          className={cn(
                            "size-1.5 shrink-0 rounded-full",
                            isFailed ? "bg-destructive" : isKeywordTriggered ? "bg-accent" : "bg-border-strong",
                          )}
                        />
                        <span
                          className={cn(
                            "min-w-0 flex-1 truncate text-sm",
                            isFailed && "text-destructive",
                          )}
                          title={truncated ? mainText : undefined}
                        >
                          {preview}
                        </span>
                        <span className="shrink-0 text-muted-foreground text-xs tabular-nums">
                          {formatHistoryTime(item.timestampMs)}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}

              <div className="flex justify-end">
                <Button
                  type="button"
                  variant="danger"
                  onClick={handleClearHistory}
                  disabled={historyItems.length === 0}
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
                <dl className="divide-y divide-border overflow-hidden rounded-2xl border border-border text-sm">
                  {[
                    { label: t("about.appName"), value: appInfo.name },
                    { label: t("about.version"), value: appInfo.version },
                    { label: t("about.buildDate"), value: appInfo.buildDate },
                    { label: t("about.author"), value: "youtonghy" },
                    {
                      label: t("about.website"),
                      value: (
                        <a
                          href="https://vtt.tokisantike.net/"
                          target="_blank"
                          rel="noopener noreferrer"
                        >
                          https://vtt.tokisantike.net/
                        </a>
                      ),
                    },
                  ].map(({ label, value }) => (
                    <div key={label} className="flex items-center justify-between gap-4 px-4 py-3">
                      <dt className="text-muted-foreground">{label}</dt>
                      <dd className="m-0 min-w-0 truncate text-right font-medium">{value}</dd>
                    </div>
                  ))}
                </dl>
              </SettingsCard>
            ) : null
          ) : null}
        </section>
          </div>
        </AnimatedSidebarInset>
      </AnimatedSidebarProvider>
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
