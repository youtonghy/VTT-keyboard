import { Info } from "lucide-react";
import { Tooltip } from "./components/Tooltip";
import { PromptTemplateEditor } from "./components/PromptTemplateEditor";
import { NumberWheelInput } from "./components/NumberWheelInput";
import { SegmentedControl } from "./components/SegmentedControl";
import { CustomSelect } from "./components/CustomSelect";
import { Toaster, toast } from "sonner";

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { getName, getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { Sidebar } from "./components/Sidebar";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { SettingsCard } from "./components/SettingsCard";
import { TagInput } from "./components/TagInput";
import { TitleBar } from "./components/TitleBar";
import { useAutostart } from "./hooks/useAutostart";
import { usePersistentBoolean } from "./hooks/usePersistentBoolean";
import { useSenseVoice } from "./hooks/useSenseVoice";
import { useSettings } from "./hooks/useSettings";
import type { Settings } from "./types/settings";

const listToString = (values: string[]) => values.join(", ");
const parseList = (value: string) =>
  value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);

const modifierKeys = new Set(["Shift", "Control", "Alt", "Meta"]);
const DEFAULT_SENSEVOICE_MODEL_ID = "FunAudioLLM/SenseVoiceSmall";
const DEFAULT_VOXTRAL_MODEL_ID = "mistralai/Voxtral-Mini-4B-Realtime-2602";
const DEFAULT_QWEN3_ASR_MODEL_ID = "Qwen/Qwen3-ASR-1.7B";
const QWEN3_ASR_CUSTOM_VARIANT = "__custom__";
const QWEN3_ASR_MODEL_VARIANTS = [
  { value: "Qwen/Qwen3-ASR-1.7B", labelKey: "sensevoice.qwenVariant17b" },
  { value: "Qwen/Qwen3-ASR-0.6B", labelKey: "sensevoice.qwenVariant06b" },
  {
    value: "Qwen/Qwen3-ForcedAligner-0.6B",
    labelKey: "sensevoice.qwenVariantForcedAligner",
  },
] as const;

const logDebug = (..._args: unknown[]) => {};

const logError = (..._args: unknown[]) => {};

const toErrorMessage = (error: unknown) => {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};

const isConflictError = (message: string) => {
  const lowered = message.toLowerCase();
  return (
    lowered.includes("already") ||
    lowered.includes("registered") ||
    lowered.includes("conflict") ||
    lowered.includes("in use")
  );
};

const normalizeShortcutKey = (key: string) => {
  if (key === " ") {
    return "Space";
  }
  if (key.startsWith("Arrow")) {
    return key.replace("Arrow", "");
  }
  if (key.length === 1) {
    return key.toUpperCase();
  }
  return key;
};

const buildShortcut = (event: KeyboardEvent) => {
  const parts: string[] = [];
  if (event.metaKey || event.ctrlKey) {
    parts.push("CommandOrControl");
  }
  if (event.altKey) {
    parts.push("Alt");
  }
  if (event.shiftKey) {
    parts.push("Shift");
  }
  const key = normalizeShortcutKey(event.key);
  parts.push(key);
  return parts.join("+");
};

const createId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;

const normalizeLocalModel = (value: string | undefined) => {
  if (value === "voxtral") {
    return "voxtral";
  }
  if (value === "qwen3-asr") {
    return "qwen3-asr";
  }
  return "sensevoice";
};

const isCudaOnlyLocalModel = (localModel: string | undefined) => {
  const normalized = normalizeLocalModel(localModel);
  return normalized === "voxtral" || normalized === "qwen3-asr";
};

const normalizeSenseVoiceDevice = (
  localModel: string | undefined,
  device: string | undefined
) => {
  if (isCudaOnlyLocalModel(localModel)) {
    return "cuda";
  }
  if (device === "cpu" || device === "cuda") {
    return device;
  }
  return "auto";
};

const getDefaultModelId = (localModel: string) => {
  const normalized = normalizeLocalModel(localModel);
  if (normalized === "voxtral") {
    return DEFAULT_VOXTRAL_MODEL_ID;
  }
  if (normalized === "qwen3-asr") {
    return DEFAULT_QWEN3_ASR_MODEL_ID;
  }
  return DEFAULT_SENSEVOICE_MODEL_ID;
};

const normalizeSenseVoiceModelId = (localModel: string, modelId: string | undefined) => {
  const trimmed = modelId?.trim();
  if (trimmed) {
    return trimmed;
  }
  return getDefaultModelId(localModel);
};

const getQwenVariantByModelId = (modelId: string | undefined) => {
  const trimmed = modelId?.trim();
  if (!trimmed) {
    return DEFAULT_QWEN3_ASR_MODEL_ID;
  }
  const matched = QWEN3_ASR_MODEL_VARIANTS.find((option) => option.value === trimmed);
  return matched ? matched.value : QWEN3_ASR_CUSTOM_VARIANT;
};

function App() {
  const { t, i18n } = useTranslation();
  const { settings, loading, saveSettings } = useSettings();
  const { syncAutostart } = useAutostart();
  const autostartSyncedOnStartup = useRef(false);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [activeSection, setActiveSection] = useState("general");
  const [sidebarCollapsed, setSidebarCollapsed] = usePersistentBoolean(
    "vtt.sidebar.collapsed",
    false
  );
  const isSenseVoiceActive = activeSection === "speech" && draft?.provider === "sensevoice";
  const {
    status: sensevoiceStatus,
    progress: sensevoiceProgress,
    logLines: sensevoiceLogLines,
    loading: sensevoiceLoading,
    refreshStatus: refreshSenseVoiceStatus,
    prepare: prepareSenseVoice,
    updateSettings: updateSenseVoiceSettings,
    startService: startSenseVoiceService,
    stopService: stopSenseVoiceService,
  } = useSenseVoice(isSenseVoiceActive);
  const [isCapturing, setIsCapturing] = useState(false);
  const [appInfo, setAppInfo] = useState<{
    name: string;
    version: string;
    buildDate: string;
  } | null>(null);

  useEffect(() => {
    if (settings) {
      const normalizedLocalModel = normalizeLocalModel(settings.sensevoice.localModel);
      setDraft({
        ...settings,
        sensevoice: {
          ...settings.sensevoice,
          localModel: normalizedLocalModel,
          modelId: normalizeSenseVoiceModelId(
            normalizedLocalModel,
            settings.sensevoice.modelId
          ),
          device: normalizeSenseVoiceDevice(
            normalizedLocalModel,
            settings.sensevoice.device
          ),
        },
      });
    }
  }, [settings]);

  useEffect(() => {
    if (!settings || autostartSyncedOnStartup.current) {
      return;
    }
    autostartSyncedOnStartup.current = true;
    void syncAutostart(settings.startup.launchOnBoot).catch((error) => {
      toast.error(t("general.launchOnBootSyncError", { error: toErrorMessage(error) }));
    });
  }, [settings, syncAutostart, t]);

  useEffect(() => {
    const fetchAppInfo = async () => {
      const [name, version, info] = await Promise.all([
        getName(),
        getVersion(),
        invoke<{ buildDate: string }>("get_app_info"),
      ]);
      setAppInfo({ name, version, buildDate: info.buildDate });
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
    if (!draft) {
      return;
    }
    let active = true;

    const registerShortcut = async () => {
      try {
        await unregisterAll();
        logDebug("unregister all success");
      } catch (error) {
        logError("unregister all failed", error);
        toast.error(t("shortcut.unregisterError"));
      }

      try {
        await register(draft.shortcut.key, (event: { state: string }) => {
          if (!active) {
            return;
          }
          logDebug("event", event.state);
          if (event.state === "Pressed") {
            invoke("start_recording")
              .then(() => logDebug("start_recording ok"))
              .catch((error) => {
                const message = toErrorMessage(error);
                logError("start_recording failed", message);
                toast.error(t("shortcut.startError", { error: message }));
              });
          }
          if (event.state === "Released") {
            invoke("stop_recording")
              .then(() => logDebug("stop_recording ok"))
              .catch((error) => {
                const message = toErrorMessage(error);
                logError("stop_recording failed", message);
                toast.error(t("shortcut.stopError", { error: message }));
              });
          }
        });
        logDebug("register success", draft.shortcut.key);
      } catch (error) {
        const message = toErrorMessage(error);
        logError("register failed", message);
        if (isConflictError(message)) {
          toast.error(t("shortcut.conflict", { shortcut: draft.shortcut.key }));
        } else {
          toast.error(t("shortcut.registerError", { error: message }));
        }
      }
    };

    void registerShortcut();

    return () => {
      active = false;
      unregisterAll()
        .then(() => logDebug("unregister all cleanup"))
        .catch((error) => logError("unregister cleanup failed", error));
    };
  }, [draft?.shortcut.key, t]);

  useEffect(() => {
    if (!isCapturing) {
      return;
    }
    const handleKeydown = (event: KeyboardEvent) => {
      if (modifierKeys.has(event.key)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const hasModifier = event.ctrlKey || event.metaKey || event.altKey || event.shiftKey;
      if (!hasModifier) {
        toast.error(t("shortcut.requireModifier"));
        setIsCapturing(false);
        return;
      }
      const shortcut = buildShortcut(event);
      updateDraft((prev) => ({
        ...prev,
        shortcut: { ...prev.shortcut, key: shortcut },
      }));
      setIsCapturing(false);
      toast.success(t("shortcut.captureSuccess", { shortcut }));
    };
    window.addEventListener("keydown", handleKeydown, true);
    return () => window.removeEventListener("keydown", handleKeydown, true);
  }, [isCapturing, t]);

  useEffect(() => {
    void invoke("set_tray_menu", {
      labels: {
        showSettings: t("tray.showSettings"),
        quit: t("tray.quit"),
      },
    });
  }, [i18n.language, t]);

  const navItems = useMemo(
    () => [
      { id: "general", label: t("nav.general") },
      { id: "shortcut", label: t("nav.shortcut") },
      { id: "recording", label: t("nav.recording") },
      { id: "speech", label: t("nav.speech") },
      { id: "text", label: t("nav.text") },
      { id: "triggers", label: t("nav.triggers") },
      { id: "about", label: t("nav.about") },
    ],
    [t]
  );

  const updateDraft = (updater: (prev: Settings) => Settings) => {
    setDraft((prev) => (prev ? updater(prev) : prev));
  };

  const validateTriggers = (next: Settings) => {
    const invalidRange = next.triggers.find((card) =>
      card.variables.every((value) => value.trim().length === 0)
    );
    if (invalidRange) {
      return t("triggers.validationVariables", { title: invalidRange.title });
    }
    const invalidKeyword = next.triggers.find((card) => card.keyword.trim().length === 0);
    if (invalidKeyword) {
      return t("triggers.validationKeyword", { title: invalidKeyword.title });
    }
    const invalidKeywordPlaceholder = next.triggers.find((card) => {
      const count = card.keyword.split("{value}").length - 1;
      return count > 1;
    });
    if (invalidKeywordPlaceholder) {
      return t("triggers.validationKeywordPlaceholder", {
        title: invalidKeywordPlaceholder.title,
      });
    }
    return null;
  };

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

  const handleSave = async () => {
    if (!draft) {
      return;
    }
    const error = validateTriggers(draft);
    if (error) {
      toast.error(error);
      return;
    }
    try {
      await saveSettings(draft);
      try {
        await syncAutostart(draft.startup.launchOnBoot);
      } catch (error) {
        toast.error(t("general.launchOnBootSyncError", { error: toErrorMessage(error) }));
        return;
      }
      toast.success(t("actions.saveSuccess"));
    } catch (err) {
      toast.error(t("actions.saveError"));
    }
  };

  const handleImport = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const selected = Array.isArray(path) ? path[0] : path;
    if (!selected) {
      return;
    }
    try {
      const data = await invoke<Settings>("import_settings", { path: selected });
      setDraft(data);
      toast.success(t("data.importSuccess"));
    } catch (err) {
      toast.error(t("data.importError"));
    }
  };

  const handleExport = async () => {
    const path = await save({
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      return;
    }
    try {
      await invoke("export_settings", { path });
      toast.success(t("data.exportSuccess"));
    } catch (err) {
      toast.error(t("data.exportError"));
    }
  };

  useEffect(() => {
    if (!draft || !settings) {
      return;
    }
    // Avoid saving if no changes
    if (JSON.stringify(draft) === JSON.stringify(settings)) {
      return;
    }

    const timer = setTimeout(() => {
      handleSave();
    }, 1000);

    return () => clearTimeout(timer);
  }, [draft, settings]);

  useEffect(() => {
    if (!isSenseVoiceActive) {
      return;
    }
    void refreshSenseVoiceStatus().catch(() => {});
  }, [isSenseVoiceActive, refreshSenseVoiceStatus]);

  const handleSenseVoicePrepare = async () => {
    if (!draft) {
      return;
    }
    try {
      await updateSenseVoiceSettings(draft.sensevoice);
    } catch (error) {
      toast.error(t("sensevoice.configSaveError", { error: toErrorMessage(error) }));
      return;
    }
    try {
      await prepareSenseVoice();
      await refreshSenseVoiceStatus();
      toast.success(t("sensevoice.prepareQueued"));
    } catch (error) {
      toast.error(t("sensevoice.prepareError", { error: toErrorMessage(error) }));
    }
  };

  const handleSenseVoiceStart = async () => {
    if (!draft) {
      return;
    }
    try {
      await updateSenseVoiceSettings(draft.sensevoice);
    } catch (error) {
      toast.error(t("sensevoice.configSaveError", { error: toErrorMessage(error) }));
      return;
    }
    try {
      await startSenseVoiceService();
      await refreshSenseVoiceStatus();
      toast.success(t("sensevoice.startQueued"));
    } catch (error) {
      toast.error(t("sensevoice.startError", { error: toErrorMessage(error) }));
    }
  };

  const handleSenseVoiceStop = async () => {
    if (!draft) {
      return;
    }
    try {
      await updateSenseVoiceSettings(draft.sensevoice);
    } catch (error) {
      toast.error(t("sensevoice.configSaveError", { error: toErrorMessage(error) }));
      return;
    }
    try {
      await stopSenseVoiceService();
      await refreshSenseVoiceStatus();
      toast.success(t("sensevoice.stopSuccess"));
    } catch (error) {
      toast.error(t("sensevoice.stopError", { error: toErrorMessage(error) }));
    }
  };

  if (loading || !draft) {
    return (
      <>

        <Toaster position="top-center" expand={false} theme={draft?.appearance?.theme === "dark" ? "dark" : draft?.appearance?.theme === "light" ? "light" : "system"} />

        <TitleBar />
        <main className="container loading">
          <p>{t("app.loading")}</p>
        </main>
      </>
    );
  }

  return (
    <>

      <Toaster position="top-center" expand={false} theme={draft?.appearance?.theme === "dark" ? "dark" : draft?.appearance?.theme === "light" ? "light" : "system"} />

      <TitleBar />
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
    }))
  }
  options={[
    { value: "openai", label: "OpenAI" },
    { value: "volcengine", label: t("speech.volcengine") },
    { value: "sensevoice", label: t("speech.sensevoice") }
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
    { value: "ko-KR", label: t("volcengine.langKoKR") }
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

                {draft.provider === "sensevoice" ? (
                  <SettingsCard title={t("speech.sensevoice")}>
                    {(() => {
                      const installed = sensevoiceStatus.installed;
                      const running = sensevoiceStatus.running;
                      const state = sensevoiceStatus.downloadState || draft.sensevoice.downloadState;
                      const lastError = sensevoiceStatus.lastError || draft.sensevoice.lastError;
                      const progressStage = sensevoiceProgress?.stage ?? "";
                      const stageLabelKey =
                        progressStage === "verify"
                          ? "started"
                          : progressStage === "warmup"
                            ? "warmup"
                            : progressStage === "done"
                              ? "ready"
                              : progressStage === "error"
                                ? "error"
                                : running && state === "running"
                                  ? "warmup"
                                  : "";
                      const prepareBusy =
                        sensevoiceLoading ||
                        progressStage === "prepare" ||
                        progressStage === "install";
                      const startBusy =
                        sensevoiceLoading ||
                        progressStage === "prepare" ||
                        progressStage === "install" ||
                        // 仅在服务仍在运行时，verify/warmup 阶段才禁用启动按钮
                        // 防止停止服务后残留阶段导致按钮持续禁用
                        (running && (progressStage === "verify" || progressStage === "warmup"));
                      const stopBusy = sensevoiceLoading;
                      const selectedLocalModel = normalizeLocalModel(
                        draft.sensevoice.localModel
                      );
                      const isVoxtralSelected = selectedLocalModel === "voxtral";
                      const isQwenSelected = selectedLocalModel === "qwen3-asr";
                      const isCudaOnlySelected = isCudaOnlyLocalModel(selectedLocalModel);
                      const currentDevice = normalizeSenseVoiceDevice(
                        selectedLocalModel,
                        draft.sensevoice.device
                      );
                      const selectedQwenVariant = getQwenVariantByModelId(
                        draft.sensevoice.modelId
                      );

                      return (
                        <>
                    <div className="sensevoice-summary">
                      <span>
                        {t("sensevoice.installed")}:{" "}
                        {installed
                          ? t("sensevoice.yes")
                          : t("sensevoice.no")}
                      </span>
                      <span>
                        {t("sensevoice.running")}:{" "}
                        {running
                          ? t("sensevoice.runningNow")
                          : t("sensevoice.stopped")}
                      </span>
                      <span>
                        {t("sensevoice.state")}:{" "}
                        {t(`sensevoice.stateMap.${state}`, {
                          defaultValue: state,
                        })}
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
          device: nextDevice,
        },
      };
    })
  }
  options={[
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
    }
  ]}
/>
                    </label>

                    {isQwenSelected ? (
                      <label className="field">
                        <span>{t("sensevoice.qwenVariant")}</span>
                        <CustomSelect
  value={selectedQwenVariant}
  onChange={(value) => {
    if (value === QWEN3_ASR_CUSTOM_VARIANT) {
      return;
    }
    updateDraft((prev) => ({
      ...prev,
      sensevoice: {
        ...prev.sensevoice,
        modelId: value,
      },
    }));
  }}
  options={[
    ...QWEN3_ASR_MODEL_VARIANTS.map((option) => ({
      value: option.value,
      label: t(option.labelKey),
    })),
    {
      value: QWEN3_ASR_CUSTOM_VARIANT,
      label: t("sensevoice.qwenVariantCustom"),
    }
  ]}
/>
                      </label>
                    ) : null}

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

                    <label className="field">
                      <span>{t("sensevoice.modelId")}</span>
                      <input
                        value={draft.sensevoice.modelId}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            sensevoice: {
                              ...prev.sensevoice,
                              modelId: event.target.value,
                            },
                          }))
                        }
                      />
                    </label>

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
	  disabled={isCudaOnlySelected}
	  options={[
	    { value: "auto", label: t("sensevoice.deviceAuto") },
	    { value: "cpu", label: t("sensevoice.deviceCpu") },
    { value: "cuda", label: t("sensevoice.deviceCuda") }
  ]}
/>
                    </label>
	                    {isVoxtralSelected ? (
	                      <div className="sensevoice-hint">
	                        {t("sensevoice.voxtralCudaOnlyHint")}
	                      </div>
	                    ) : null}
                    {isQwenSelected ? (
                      <div className="sensevoice-hint">
                        {t("sensevoice.qwenCudaOnlyHint")}
                      </div>
                    ) : null}

                    {sensevoiceProgress ? (
                      <div className="sensevoice-progress">
                        <span>{sensevoiceProgress.message}</span>
                        <span>
                          {sensevoiceProgress.percent !== undefined
                            ? `${sensevoiceProgress.percent}%`
                            : ""}
                        </span>
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
                        <div className="sensevoice-log-title">{t("sensevoice.logTitle")}</div>
                        <pre>{sensevoiceLogLines.join("\n")}</pre>
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
                        <button
                          type="button"
                          onClick={handleSenseVoiceStart}
                          disabled={startBusy}
                        >
                          {t("sensevoice.start")}
                        </button>
                      ) : null}
                      {running ? (
                        <button
                          type="button"
                          onClick={handleSenseVoiceStop}
                          disabled={stopBusy}
                        >
                          {t("sensevoice.stop")}
                        </button>
                      ) : null}
                    </div>
                        </>
                      );
                    })()}
                  </SettingsCard>
                ) : null}
              </>
            ) : null}

            {activeSection === "text" ? (
              <SettingsCard title={t("text.title")} description={t("text.description")}>
                <label className="field">
                  <span>{t("text.model")}</span>
                  <input
                    value={draft.openai.text.model}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: { ...prev.openai.text, model: event.target.value },
                        },
                      }))
                    }
                  />
                </label>
                <label className="field">
                  <span>{t("text.instructions")}</span>
                  <input
                    value={draft.openai.text.instructions}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: {
                            ...prev.openai.text,
                            instructions: event.target.value,
                          },
                        },
                      }))
                    }
                  />
                </label>
                <label className="field">
                  <span>{t("text.temperature")}</span>
                  <NumberWheelInput
  step={0.1}
  value={draft.openai.text.temperature}
  onChange={(value) =>
    updateDraft((prev) => ({
      ...prev,
      openai: {
        ...prev.openai,
        text: {
          ...prev.openai.text,
          temperature: value,
        },
      },
    }))
  }
/>
                </label>
                <label className="field">
                  <span>{t("text.maxOutputTokens")}</span>
                  <NumberWheelInput
  min={1}
  value={draft.openai.text.maxOutputTokens}
  onChange={(value) =>
    updateDraft((prev) => ({
      ...prev,
      openai: {
        ...prev.openai,
        text: {
          ...prev.openai.text,
          maxOutputTokens: value,
        },
      },
    }))
  }
/>
                </label>
                <label className="field">
                  <span>{t("text.topP")}</span>
                  <NumberWheelInput
  step={0.1}
  value={draft.openai.text.topP}
  onChange={(value) =>
    updateDraft((prev) => ({
      ...prev,
      openai: {
        ...prev.openai,
        text: {
          ...prev.openai.text,
          topP: value,
        },
      },
    }))
  }
/>
                </label>
              </SettingsCard>
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
    </>
  );
}

export default App;
