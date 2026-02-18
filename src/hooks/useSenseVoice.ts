import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SenseVoiceSettings } from "../types/settings";

export interface SenseVoiceStatus {
  installed: boolean;
  enabled: boolean;
  running: boolean;
  serviceUrl: string;
  modelId: string;
  device: string;
  downloadState: string;
  lastError: string;
}

export interface SenseVoiceProgress {
  stage: string;
  message: string;
  percent?: number;
  detail?: string;
}

const defaultStatus: SenseVoiceStatus = {
  installed: false,
  enabled: false,
  running: false,
  serviceUrl: "",
  modelId: "",
  device: "auto",
  downloadState: "idle",
  lastError: "",
};

export function useSenseVoice() {
  const [status, setStatus] = useState<SenseVoiceStatus>(defaultStatus);
  const [progress, setProgress] = useState<SenseVoiceProgress | null>(null);
  const [logLines, setLogLines] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    const next = await invoke<SenseVoiceStatus>("get_sensevoice_status");
    setStatus(next);
    return next;
  }, []);

  const prepare = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("prepare_sensevoice");
      setStatus(next);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  const updateSettings = useCallback(async (sensevoice: SenseVoiceSettings) => {
    await invoke("update_sensevoice_settings", { sensevoice });
  }, []);

  const startService = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("start_sensevoice_service");
      setStatus(next);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  const stopService = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("stop_sensevoice_service");
      setStatus(next);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshStatus().catch(() => {});
  }, [refreshStatus]);

  useEffect(() => {
    const unlisten = listen<SenseVoiceProgress>("sensevoice-progress", (event) => {
      const payload = event.payload;
      setProgress(payload);
      if (payload.stage === "prepare" && payload.percent === 5) {
        setLogLines([]);
      }
      if (payload.detail && payload.detail.trim().length > 0) {
        setLogLines((prev) => {
          const next = [...prev, payload.detail!.trim()];
          return next.slice(-100);
        });
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  return {
    status,
    progress,
    logLines,
    loading,
    refreshStatus,
    prepare,
    updateSettings,
    startService,
    stopService,
  };
}
