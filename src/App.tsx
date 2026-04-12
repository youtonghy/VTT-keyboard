import { Info } from "lucide-react";
import { Tooltip } from "./components/Tooltip";
import { PromptTemplateEditor } from "./components/PromptTemplateEditor";
import { NumberWheelInput } from "./components/NumberWheelInput";
import { SegmentedControl } from "./components/SegmentedControl";
import { Toaster, toast } from "sonner";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getName, getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { Sidebar } from "./components/Sidebar";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { SettingsCard } from "./components/SettingsCard";
import { SpeechSettingsSection } from "./components/settings/SpeechSettingsSection";
import { TextProcessingSettingsSection } from "./components/settings/TextProcessingSettingsSection";
import { TagInput } from "./components/TagInput";
import { TitleBar } from "./components/TitleBar";
import { useAutostart } from "./hooks/useAutostart";
import { usePersistentBoolean } from "./hooks/usePersistentBoolean";
import { useSenseVoice } from "./hooks/useSenseVoice";
import { useSettings } from "./hooks/useSettings";
import { useUpdater } from "./hooks/useUpdater";
import { HistoryDetailDialog } from "./components/HistoryDetailDialog";
import type { TranscriptionHistoryItem } from "./types/history";
import type { Settings } from "./types/settings";

const parseList = (value: string) =>
  value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
const normalizeAliyunRegion = (value: string | undefined) =>
  value === "singapore" ? "singapore" : "beijing";

const modifierKeys = new Set(["Shift", "Control", "Alt", "Meta"]);
const DEFAULT_SENSEVOICE_MODEL_ID = "FunAudioLLM/SenseVoiceSmall";
const DEFAULT_SHERPA_ONNX_SENSEVOICE_MODEL_ID =
  "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
const DEFAULT_VOXTRAL_MODEL_ID = "mistralai/Voxtral-Mini-4B-Realtime-2602";
const DEFAULT_QWEN3_ASR_MODEL_ID = "Qwen/Qwen3-ASR-1.7B";
const SHERPA_LANGUAGE_OPTIONS = [
  { value: "auto", labelKey: "sensevoice.languageAuto" },
  { value: "zh", labelKey: "sensevoice.languageZh" },
  { value: "en", labelKey: "sensevoice.languageEn" },
  { value: "ja", labelKey: "sensevoice.languageJa" },
  { value: "ko", labelKey: "sensevoice.languageKo" },
  { value: "yue", labelKey: "sensevoice.languageYue" },
] as const;
const QWEN3_ASR_MODEL_VARIANTS = [
  { value: "Qwen/Qwen3-ASR-1.7B", labelKey: "sensevoice.qwenVariant17b" },
  { value: "Qwen/Qwen3-ASR-0.6B", labelKey: "sensevoice.qwenVariant06b" },
  {
    value: "Qwen/Qwen3-ForcedAligner-0.6B",
    labelKey: "sensevoice.qwenVariantForcedAligner",
  },
] as const;
const MAX_HISTORY_ITEMS = 200;
const HISTORY_PREVIEW_MAX_CHARS = 50;

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

const normalizeStopMode = (value: string | undefined): "stop" | "pause" => {
  if (value === "pause") {
    return "pause";
  }
  return "stop";
};

const isCudaOnlyLocalModel = (localModel: string | undefined) => {
  const normalized = normalizeLocalModel(localModel);
  return normalized === "voxtral" || normalized === "qwen3-asr";
};

const isSherpaLocalModel = (localModel: string | undefined) =>
  normalizeLocalModel(localModel) === "sherpa-onnx-sensevoice";

const normalizeSenseVoiceDevice = (
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

const getDefaultModelId = (localModel: string) => {
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

const normalizeSenseVoiceModelId = (localModel: string, modelId: string | undefined) => {
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

const normalizeSenseVoiceLanguage = (language: string | undefined) => {
  if (language === "zh" || language === "en" || language === "ja" || language === "ko" || language === "yue") {
    return language;
  }
  return "auto";
};

const formatBytes = (value: number | undefined) => {
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

const getQwenVariantByModelId = (modelId: string | undefined) => {
  const trimmed = modelId?.trim();
  if (!trimmed) {
    return DEFAULT_QWEN3_ASR_MODEL_ID;
  }
  const matched = QWEN3_ASR_MODEL_VARIANTS.find((option) => option.value === trimmed);
  return matched ? matched.value : DEFAULT_QWEN3_ASR_MODEL_ID;
};

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
  const { settings, loading, saveSettings } = useSettings();
  const { syncAutostart } = useAutostart();
  const autostartSyncedOnStartup = useRef(false);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [activeSection, setActiveSection] = useState("general");
  const [sidebarCollapsed, setSidebarCollapsed] = usePersistentBoolean(
    "vtt.sidebar.collapsed",
    false
  );
  const [sensevoiceLogsExpanded, setSensevoiceLogsExpanded] = usePersistentBoolean(
    "vtt.sensevoice.logs.expanded",
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
  const updater = useUpdater();
  const [isCapturing, setIsCapturing] = useState(false);
  const [appInfo, setAppInfo] = useState<
    ({ name: string; version: string } & AppInfoPayload) | null
  >(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyItems, setHistoryItems] = useState<TranscriptionHistoryItem[]>([]);
  const [selectedHistoryItem, setSelectedHistoryItem] =
    useState<TranscriptionHistoryItem | null>(null);
  const [pendingSherpaAutoStart, setPendingSherpaAutoStart] = useState(false);
  const supportsSherpaOnnxSenseVoice =
    appInfo?.supportsSherpaOnnxSenseVoice ?? true;
  const sherpaFallbackActive =
    !supportsSherpaOnnxSenseVoice &&
    normalizeLocalModel(settings?.sensevoice.localModel) === "sherpa-onnx-sensevoice";

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
    if (settings) {
      const normalizedLocalModel = normalizeLocalModel(settings.sensevoice.localModel);
      const effectiveLocalModel =
        !supportsSherpaOnnxSenseVoice &&
        normalizedLocalModel === "sherpa-onnx-sensevoice"
          ? "sensevoice"
          : normalizedLocalModel;
      const normalizedAliyunRegion = normalizeAliyunRegion(settings.aliyun.region);
      const effectiveAliyunRegion =
        settings.provider === "aliyun-paraformer" ? "beijing" : normalizedAliyunRegion;
      setDraft({
        ...settings,
        sensevoice: {
          ...settings.sensevoice,
          localModel: effectiveLocalModel,
          stopMode: normalizeStopMode(settings.sensevoice.stopMode),
          modelId: normalizeSenseVoiceModelId(
            effectiveLocalModel,
            settings.sensevoice.modelId
          ),
          language: normalizeSenseVoiceLanguage(settings.sensevoice.language),
          device: normalizeSenseVoiceDevice(
            effectiveLocalModel,
            settings.sensevoice.device
          ),
        },
        aliyun: {
          ...settings.aliyun,
          region: effectiveAliyunRegion,
          apiKeys: {
            beijing: settings.aliyun.apiKeys.beijing ?? "",
            singapore: settings.aliyun.apiKeys.singapore ?? "",
          },
          asr: {
            ...settings.aliyun.asr,
            vocabularyId: settings.aliyun.asr.vocabularyId ?? "",
          },
          paraformer: {
            ...settings.aliyun.paraformer,
            vocabularyId: settings.aliyun.paraformer.vocabularyId ?? "",
            languageHints: settings.aliyun.paraformer.languageHints ?? [],
          },
        },
      });
    }
  }, [settings, supportsSherpaOnnxSenseVoice]);

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
    const nextSettings = buildPersistedSettings();
    if (!nextSettings) {
      return;
    }
    const error = validateTriggers(nextSettings);
    if (error) {
      toast.error(error);
      return;
    }
    try {
      await saveSettings(nextSettings);
      try {
        await syncAutostart(nextSettings.startup.launchOnBoot);
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

  // Always keep ref pointing to the latest handleSave to avoid stale-closure in the debounce
  const handleSaveRef = useRef(handleSave);
  handleSaveRef.current = handleSave;

  useEffect(() => {
    if (!draft || !settings) {
      return;
    }
    // Avoid saving if no changes
    if (JSON.stringify(draft) === JSON.stringify(settings)) {
      return;
    }

    const timer = setTimeout(() => {
      handleSaveRef.current();
    }, 1000);

    return () => clearTimeout(timer);
  }, [draft, settings]);

  useEffect(() => {
    if (!isSenseVoiceActive) {
      return;
    }
    void refreshSenseVoiceStatus().catch(() => {});
  }, [isSenseVoiceActive, refreshSenseVoiceStatus]);

  const buildPersistedSenseVoiceSettings = useCallback(() => {
    if (!draft) {
      return null;
    }
    return {
      ...draft.sensevoice,
      enabled: sensevoiceStatus.enabled,
      installed: sensevoiceStatus.installed,
      downloadState: sensevoiceStatus.downloadState,
      lastError: sensevoiceStatus.lastError,
    };
  }, [draft, sensevoiceStatus]);

  const buildPersistedSettings = useCallback(() => {
    if (!draft) {
      return null;
    }
    const nextSenseVoiceSettings = {
      ...draft.sensevoice,
      enabled: sensevoiceStatus.enabled,
      installed: sensevoiceStatus.installed,
      downloadState: sensevoiceStatus.downloadState,
      lastError: sensevoiceStatus.lastError,
    };
    return {
      ...draft,
      sensevoice: nextSenseVoiceSettings,
    };
  }, [draft, sensevoiceStatus]);

  const handleSenseVoicePrepare = async () => {
    const nextSenseVoiceSettings = buildPersistedSenseVoiceSettings();
    if (!nextSenseVoiceSettings) {
      return;
    }
    try {
      await updateSenseVoiceSettings(nextSenseVoiceSettings);
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
    const nextSenseVoiceSettings = buildPersistedSenseVoiceSettings();
    if (!nextSenseVoiceSettings) {
      return;
    }
    try {
      await updateSenseVoiceSettings(nextSenseVoiceSettings);
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
    const nextSenseVoiceSettings = buildPersistedSenseVoiceSettings();
    if (!nextSenseVoiceSettings || !draft) {
      return;
    }
    try {
      await updateSenseVoiceSettings(nextSenseVoiceSettings);
    } catch (error) {
      toast.error(t("sensevoice.configSaveError", { error: toErrorMessage(error) }));
      return;
    }
    try {
      await stopSenseVoiceService();
      await refreshSenseVoiceStatus();
      const runtimeKind = sensevoiceStatus.runtimeKind;
      const stopMode = normalizeStopMode(draft.sensevoice.stopMode);
      if (runtimeKind === "native") {
        toast.success(t("sensevoice.unloadSuccess"));
      } else if (stopMode === "pause") {
        toast.success(t("sensevoice.pauseSuccess"));
      } else {
        toast.success(t("sensevoice.stopSuccess"));
      }
    } catch (error) {
      toast.error(t("sensevoice.stopError", { error: toErrorMessage(error) }));
    }
  };

  useEffect(() => {
    const unlisten = listen("sensevoice-startup-download-required", async () => {
      const confirmed = window.confirm(t("sensevoice.startupDownloadPrompt"));
      if (!confirmed) {
        return;
      }
      setPendingSherpaAutoStart(true);
      await handleSenseVoicePrepare();
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [t, handleSenseVoicePrepare]);

  useEffect(() => {
    if (!pendingSherpaAutoStart || !draft || !supportsSherpaOnnxSenseVoice) {
      if (!supportsSherpaOnnxSenseVoice) {
        setPendingSherpaAutoStart(false);
      }
      return;
    }
    if (normalizeLocalModel(draft.sensevoice.localModel) !== "sherpa-onnx-sensevoice") {
      setPendingSherpaAutoStart(false);
      return;
    }
    if (!sensevoiceStatus.installed || sensevoiceStatus.running) {
      return;
    }
    if (sensevoiceStatus.downloadState !== "ready") {
      return;
    }
    setPendingSherpaAutoStart(false);
    void handleSenseVoiceStart();
  }, [
    draft,
    handleSenseVoiceStart,
    pendingSherpaAutoStart,
    sensevoiceStatus,
    supportsSherpaOnnxSenseVoice,
  ]);

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
                normalizeLocalModel={normalizeLocalModel}
                normalizeSenseVoiceLanguage={normalizeSenseVoiceLanguage}
                normalizeSenseVoiceDevice={normalizeSenseVoiceDevice}
                normalizeStopMode={normalizeStopMode}
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
