import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SenseVoiceSettings } from "../types/settings";
import { LOCAL_MODEL_SENSEVOICE } from "../utils/sensevoice";

export interface SenseVoiceStatus {
  installed: boolean;
  enabled: boolean;
  running: boolean;
  runtimeState: "stopped" | "running" | "starting";
  localModel: string;
  modelId: string;
  language: string;
  downloadState: string;
  lastError: string;
}

export interface SenseVoiceProgress {
  stage: string;
  message: string;
  percent?: number;
  downloadedBytes?: number;
  totalBytes?: number;
  detail?: string;
}

const defaultStatus: SenseVoiceStatus = {
  installed: false,
  enabled: false,
  running: false,
  runtimeState: "stopped",
  localModel: LOCAL_MODEL_SENSEVOICE,
  modelId: "",
  language: "auto",
  downloadState: "idle",
  lastError: "",
};

export function useSenseVoice(monitoringEnabled = false) {
  const [status, setStatus] = useState<SenseVoiceStatus>(defaultStatus);
  const [progress, setProgress] = useState<SenseVoiceProgress | null>(null);
  const [logLines, setLogLines] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    const next = await invoke<SenseVoiceStatus>("get_sensevoice_status");
    setStatus(next);
    if (next.downloadState === "ready" || !next.running) {
      setProgress((prev) => (prev?.stage === "done" ? null : prev));
    }
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
      void refreshStatus().catch(() => {});
      return next;
    } finally {
      setLoading(false);
    }
  }, [refreshStatus]);

  const stopService = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("stop_sensevoice_service");
      setStatus(next);
      setProgress(null);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  const removeModel = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("remove_sensevoice_model");
      setStatus(next);
      setProgress(null);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshStatus().catch(() => {});
  }, [refreshStatus]);

  useEffect(() => {
    if (!monitoringEnabled) {
      return;
    }
    const timer = window.setInterval(() => {
      void refreshStatus().catch(() => {});
    }, 2000);
    return () => window.clearInterval(timer);
  }, [monitoringEnabled, refreshStatus]);

  useEffect(() => {
    const unlisten = listen<SenseVoiceProgress>("sensevoice-progress", (event) => {
      const payload = event.payload;
      // 收到 stopped 事件时清除 progress，防止残留阶段禁用启动按钮
      if (payload.stage === "stopped") {
        setProgress(null);
        void refreshStatus().catch(() => {});
        return;
      }
      setProgress(payload);
      if (payload.stage === "prepare" && payload.percent === 5) {
        setLogLines([]);
      }
      if (payload.stage === "done" || payload.stage === "error") {
        void refreshStatus().catch(() => {});
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
  }, [refreshStatus]);

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
    removeModel,
  };
}
