export type TriggerMatchMode = "keyword" | "auto";

export interface TriggerMatch {
  triggerId: string;
  triggerTitle: string;
  keyword: string;
  matchedValue: string;
  mode: TriggerMatchMode;
}

export type TranscriptionHistoryStatus = "success" | "failed";

export interface TranscriptionHistoryItem {
  id: string;
  timestampMs: number;
  status: TranscriptionHistoryStatus;
  transcriptionText: string;
  finalText: string;
  triggered: boolean;
  triggeredByKeyword: boolean;
  triggerMatches: TriggerMatch[];
  errorMessage?: string;
}
