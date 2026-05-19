import type { TranscriptionHistoryItem } from "./types/history";
import type { Settings } from "./types/settings";
import type { UpdateStatusPayload } from "./types/updater";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke?: unknown;
      metadata?: unknown;
    };
  }
}

const createDefaultSettings = (): Settings => ({
  shortcut: { key: "CommandOrControl+Shift+Space" },
  recording: { segmentSeconds: 60 },
  provider: "openai",
  openai: {
    apiBase: "https://api.openai.com/v1",
    apiKey: "",
    speechToText: {
      model: "gpt-4o-transcribe",
      language: "",
      prompt: "",
      responseFormat: "json",
      temperature: 0,
      timestampGranularities: [],
      chunkingStrategy: "auto",
      include: [],
      stream: false,
      knownSpeakerNames: [],
      knownSpeakerReferences: [],
    },
  },
  textProcessing: {
    provider: "openai",
    openai: {
      apiBase: "https://api.openai.com/v1",
      apiKey: "",
      model: "gpt-4o-mini",
      temperature: 0.6,
      maxOutputTokens: 800,
      topP: 1,
      instructions: "",
    },
  },
  volcengine: {
    appId: "",
    accessToken: "",
    useStreaming: false,
    useFast: false,
    language: "zh-CN",
  },
  sensevoice: {
    enabled: false,
    installed: true,
    localModel: "sensevoice",
    stopMode: "pause",
    serviceUrl: "http://127.0.0.1:8765",
    modelId: "FunAudioLLM/SenseVoiceSmall",
    language: "auto",
    device: "auto",
    downloadState: "ready",
    lastError: "",
  },
  aliyun: {
    region: "beijing",
    apiKeys: { beijing: "", singapore: "" },
    asr: { vocabularyId: "" },
    paraformer: { languageHints: [], vocabularyId: "" },
  },
  triggers: [
    {
      id: "translate",
      title: "Translate",
      enabled: true,
      autoApply: false,
      locked: true,
      keyword: "translate",
      promptTemplate: "Translate the following content to {value}.",
      variables: ["English"],
    },
    {
      id: "polish",
      title: "Polish",
      enabled: true,
      autoApply: false,
      locked: true,
      keyword: "polish",
      promptTemplate: "Polish the following content into {value}.",
      variables: ["spoken style"],
    },
  ],
  output: { removeNewlines: false },
  appearance: { theme: "system" },
  startup: {
    launchOnBoot: false,
    autoCheckUpdates: true,
    autoInstallUpdatesOnQuit: true,
  },
  history: { enabled: true },
});

const createHistoryItems = (): TranscriptionHistoryItem[] => [
  {
    id: "history-1",
    timestampMs: Date.now() - 1000 * 60 * 5,
    status: "success",
    transcriptionText: "这是一个用于验证历史记录列表在窄屏下换行和时间戳对齐的示例。",
    finalText: "This is a translated sample used to verify the history detail view.",
    modelGroup: "OpenAI",
    transcriptionElapsedMs: 1280,
    recordingDurationMs: 6200,
    triggered: true,
    triggeredByKeyword: true,
    triggerMatches: [
      {
        triggerId: "translate",
        triggerTitle: "Translate",
        keyword: "translate",
        matchedValue: "English",
        mode: "keyword",
      },
    ],
  },
  {
    id: "history-2",
    timestampMs: Date.now() - 1000 * 60 * 35,
    status: "failed",
    transcriptionText: "",
    finalText: "",
    modelGroup: "SenseVoice",
    transcriptionElapsedMs: 400,
    recordingDurationMs: 1800,
    triggered: false,
    triggeredByKeyword: false,
    triggerMatches: [],
    errorMessage: "Mocked network timeout for layout verification.",
  },
];

const clone = <T,>(value: T): T =>
  typeof structuredClone === "function"
    ? structuredClone(value)
    : JSON.parse(JSON.stringify(value));

let mockSettings = createDefaultSettings();
let mockAutostartEnabled = false;

const getSenseVoiceStatus = () => ({
  installed: mockSettings.sensevoice.installed,
  enabled: mockSettings.sensevoice.enabled,
  running: false,
  runtimeState: "stopped",
  runtimeKind: "docker",
  supportsPause: true,
  localModel: mockSettings.sensevoice.localModel,
  serviceUrl: mockSettings.sensevoice.serviceUrl,
  modelId: mockSettings.sensevoice.modelId,
  device: mockSettings.sensevoice.device,
  downloadState: mockSettings.sensevoice.downloadState,
  lastError: mockSettings.sensevoice.lastError,
});

const updateStatus: UpdateStatusPayload = {
  status: "idle",
  currentVersion: "0.0.0-dev",
  latestVersion: null,
  notes: null,
  pubDate: null,
  downloadedBytes: null,
  totalBytes: null,
  error: null,
};

export async function setupDevTauriMocks() {
  if (!import.meta.env.DEV || typeof window === "undefined") {
    return;
  }

  if (typeof window.__TAURI_INTERNALS__?.invoke === "function") {
    return;
  }

  const { mockIPC, mockWindows } = await import("@tauri-apps/api/mocks");

  mockWindows("main");
  mockIPC(
    (cmd, payload) => {
      switch (cmd) {
        case "get_settings":
          return clone(mockSettings);
        case "update_settings":
          mockSettings = clone((payload as { settings?: Settings } | undefined)?.settings ?? mockSettings);
          return clone(mockSettings);
        case "import_settings":
          return clone(mockSettings);
        case "export_settings":
        case "set_tray_menu":
        case "start_recording":
        case "stop_recording":
        case "install_downloaded_update":
        case "retry_update_check":
        case "dismiss_update_error":
          return null;
        case "get_app_info":
          return {
            buildDate: "dev-browser",
            platform: "macos",
            arch: "aarch64",
            supportsSherpaOnnxSenseVoice: true,
          };
        case "plugin:app|name":
          return "VTT Keyboard";
        case "plugin:app|version":
          return "0.0.0-dev";
        case "plugin:app|tauri_version":
          return "2.0.0-dev";
        case "get_update_status":
          return clone(updateStatus);
        case "get_transcription_history":
          return createHistoryItems();
        case "clear_transcription_history":
          return null;
        case "get_sensevoice_status":
        case "prepare_sensevoice":
        case "start_sensevoice_service":
        case "stop_sensevoice_service":
          return getSenseVoiceStatus();
        case "update_sensevoice_settings":
          mockSettings = {
            ...mockSettings,
            sensevoice: {
              ...mockSettings.sensevoice,
              ...(payload as { sensevoice?: Settings["sensevoice"] } | undefined)?.sensevoice,
            },
          };
          return null;
        case "update_sensevoice_runtime":
          return null;
        case "plugin:autostart|is_enabled":
          return mockAutostartEnabled;
        case "plugin:autostart|enable":
          mockAutostartEnabled = true;
          return null;
        case "plugin:autostart|disable":
          mockAutostartEnabled = false;
          return null;
        case "plugin:global-shortcut|register":
        case "plugin:global-shortcut|unregister":
        case "plugin:global-shortcut|unregister_all":
          return null;
        case "plugin:global-shortcut|is_registered":
          return true;
        case "plugin:dialog|open":
        case "plugin:dialog|save":
          return null;
        case "plugin:dialog|message":
          return null;
        case "plugin:dialog|ask":
        case "plugin:dialog|confirm":
          return false;
        case "plugin:window|get_all_windows":
          return ["main"];
        case "plugin:window|is_maximized":
        case "plugin:window|is_minimized":
        case "plugin:window|is_fullscreen":
          return false;
        case "plugin:window|is_focused":
        case "plugin:window|is_decorated":
        case "plugin:window|is_resizable":
        case "plugin:window|is_maximizable":
        case "plugin:window|is_minimizable":
        case "plugin:window|is_closable":
        case "plugin:window|is_visible":
          return true;
        case "plugin:window|scale_factor":
          return window.devicePixelRatio || 1;
        case "plugin:window|inner_size":
        case "plugin:window|outer_size":
          return { width: window.innerWidth, height: window.innerHeight };
        case "plugin:window|inner_position":
        case "plugin:window|outer_position":
        case "plugin:window|cursor_position":
          return { x: 0, y: 0 };
        default:
          if (cmd.startsWith("plugin:window|")) {
            return null;
          }
          console.warn(`[dev-tauri-mocks] Unhandled command: ${cmd}`, payload);
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
