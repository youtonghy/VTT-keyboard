export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "downloaded"
  | "installing"
  | "upToDate"
  | "error";

export interface UpdateStatusPayload {
  status: UpdateStatus;
  currentVersion: string;
  latestVersion: string | null;
  notes: string | null;
  pubDate: string | null;
  downloadedBytes: number | null;
  totalBytes: number | null;
  error: string | null;
}