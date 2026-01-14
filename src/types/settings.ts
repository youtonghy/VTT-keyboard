export type TranscriptionProvider = "openai" | "volcengine";

export interface Settings {
  shortcut: ShortcutSettings;
  recording: RecordingSettings;
  provider: TranscriptionProvider;
  openai: OpenAiSettings;
  volcengine: VolcengineSettings;
  triggers: TriggerCard[];
  appearance: AppearanceSettings;
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
  text: TextSettings;
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

export interface TextSettings {
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
  example: string;
  description: string;
}

export interface AppearanceSettings {
  theme: string;
}

export interface VolcengineSettings {
  appId: string;
  accessToken: string;
  useStreaming: boolean;
  useFast: boolean;
  language: string;
}
