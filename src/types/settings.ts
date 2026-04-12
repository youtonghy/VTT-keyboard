export type TranscriptionProvider =
  | "openai"
  | "volcengine"
  | "sensevoice"
  | "aliyun-asr"
  | "aliyun-paraformer";

export type TextProcessingProvider = "openai";

export interface Settings {
  shortcut: ShortcutSettings;
  recording: RecordingSettings;
  provider: TranscriptionProvider;
  openai: OpenAiSettings;
  textProcessing: TextProcessingSettings;
  volcengine: VolcengineSettings;
  sensevoice: SenseVoiceSettings;
  aliyun: AliyunSettings;
  triggers: TriggerCard[];
  output: OutputSettings;
  appearance: AppearanceSettings;
  startup: StartupSettings;
  history: HistorySettings;
}

export interface ShortcutSettings {
  key: string;
}

export interface RecordingSettings {
  segmentSeconds: number;
}

export interface OpenAiSettings {
  apiBase: string;
  apiKey: string;
  speechToText: SpeechToTextSettings;
}

export interface SpeechToTextSettings {
  model: string;
  language: string;
  prompt: string;
  responseFormat: string;
  temperature: number;
  timestampGranularities: string[];
  chunkingStrategy: string;
  include: string[];
  stream: boolean;
  knownSpeakerNames: string[];
  knownSpeakerReferences: string[];
}

export interface TextProcessingSettings {
  provider: TextProcessingProvider;
  openai: TextSettings;
}

export interface TextSettings {
  apiBase: string;
  apiKey: string;
  model: string;
  temperature: number;
  maxOutputTokens: number;
  topP: number;
  instructions: string;
}

export interface TriggerCard {
  id: string;
  title: string;
  enabled: boolean;
  autoApply: boolean;
  locked: boolean;
  keyword: string;
  promptTemplate: string;
  variables: string[];
}

export interface OutputSettings {
  removeNewlines: boolean;
}

export interface AppearanceSettings {
  theme: string;
}

export interface StartupSettings {
  launchOnBoot: boolean;
  autoCheckUpdates: boolean;
  autoInstallUpdatesOnQuit: boolean;
}

export interface HistorySettings {
  enabled: boolean;
}

export interface VolcengineSettings {
  appId: string;
  accessToken: string;
  useStreaming: boolean;
  useFast: boolean;
  language: string;
}

export interface SenseVoiceSettings {
  enabled: boolean;
  installed: boolean;
  localModel: string;
  stopMode: "stop" | "pause";
  serviceUrl: string;
  modelId: string;
  language: string;
  device: string;
  downloadState: string;
  lastError: string;
}

export interface AliyunSettings {
  region: "beijing" | "singapore";
  apiKeys: AliyunApiKeys;
  asr: AliyunAsrSettings;
  paraformer: AliyunParaformerSettings;
}

export interface AliyunApiKeys {
  beijing: string;
  singapore: string;
}

export interface AliyunAsrSettings {
  vocabularyId: string;
}

export interface AliyunParaformerSettings {
  languageHints: string[];
  vocabularyId: string;
}
