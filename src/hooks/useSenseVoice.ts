import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SenseVoiceSettings } from "../types/settings";

export interface SenseVoiceStatus {
  installed: boolean;
  enabled: boolean;
  running: boolean;
  runtimeState: "stopped" | "running" | "paused" | "starting" | "exited";
  runtimeKind: "native" | "docker";
  supportsPause: boolean;
  localModel: string;
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
  downloadedBytes?: number;
  totalBytes?: number;
  detail?: string;
}

export interface SenseVoiceDockerRuntimeStatus {
  available: boolean;
  daemonRunning: boolean;
  containerName: string;
  containerExists: boolean;
  containerState: string;
  containerModelKey: string;
  containerModelId: string;
  expectedModelKey: string;
  expectedModelId: string;
  imageTag: string;
  imageExists: boolean;
  serviceUrl: string;
  runtimeDir: string;
  modelsDir: string;
  lastError: string;
}

interface SenseVoiceRuntimeLog {
  stream: "stdout" | "stderr";
  line: string;
  ts?: number;
}

const defaultStatus: SenseVoiceStatus = {
  installed: false,
  enabled: false,
  running: false,
  runtimeState: "stopped",
  runtimeKind: "docker",
  supportsPause: true,
  localModel: "sensevoice",
  serviceUrl: "",
  modelId: "",
  device: "auto",
  downloadState: "idle",
  lastError: "",
};

const defaultDockerRuntimeStatus: SenseVoiceDockerRuntimeStatus = {
  available: false,
  daemonRunning: false,
  containerName: "",
  containerExists: false,
  containerState: "stopped",
  containerModelKey: "",
  containerModelId: "",
  expectedModelKey: "sensevoice",
  expectedModelId: "",
  imageTag: "",
  imageExists: false,
  serviceUrl: "",
  runtimeDir: "",
  modelsDir: "",
  lastError: "",
};

export function useSenseVoice(monitoringEnabled = false) {
  const [status, setStatus] = useState<SenseVoiceStatus>(defaultStatus);
  const [dockerRuntimeStatus, setDockerRuntimeStatus] = useState<SenseVoiceDockerRuntimeStatus>(
    defaultDockerRuntimeStatus
  );
  const [progress, setProgress] = useState<SenseVoiceProgress | null>(null);
  const [logLines, setLogLines] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    const next = await invoke<SenseVoiceStatus>("get_sensevoice_status");
    setStatus(next);
    if (next.downloadState === "ready" || !next.running) {
      setProgress((prev) => {
        if (!prev) {
          return prev;
        }
        if (prev.stage === "verify" || prev.stage === "warmup") {
          return null;
        }
        return prev;
      });
    }
    return next;
  }, []);

  const refreshDockerRuntimeStatus = useCallback(async () => {
    const next = await invoke<SenseVoiceDockerRuntimeStatus>("get_sensevoice_docker_runtime_status");
    setDockerRuntimeStatus(next);
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
      // 主动清除 progress，防止残留的 verify/warmup 阶段导致启动按钮被禁用
      setProgress(null);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  const stopServiceForce = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("stop_sensevoice_service_force");
      setStatus(next);
      setProgress(null);
      return next;
    } finally {
      setLoading(false);
    }
  }, []);

  const updateRuntime = useCallback(async () => {
    setLoading(true);
    try {
      await invoke("update_sensevoice_runtime");
    } finally {
      setLoading(false);
    }
  }, []);

  const restartService = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("restart_sensevoice_service");
      setStatus(next);
      void refreshStatus().catch(() => {});
      return next;
    } finally {
      setLoading(false);
    }
  }, [refreshStatus]);

  const removeRuntimeContainer = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invoke<SenseVoiceStatus>("remove_sensevoice_runtime_container");
      setStatus(next);
      setProgress(null);
      void refreshDockerRuntimeStatus().catch(() => {});
      return next;
    } finally {
      setLoading(false);
    }
  }, [refreshDockerRuntimeStatus]);

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
      if (payload.stage === "stopped" || payload.stage === "paused") {
        setProgress(null);
        void refreshStatus().catch(() => {});
        return;
      }
      setProgress(payload);
      if (payload.stage === "prepare" && payload.percent === 5) {
        setLogLines([]);
      }
      if (
        payload.stage === "verify" ||
        payload.stage === "done" ||
        payload.stage === "error"
      ) {
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

  useEffect(() => {
    const unlisten = listen<SenseVoiceRuntimeLog>("sensevoice-runtime-log", (event) => {
      const payload = event.payload;
      const rawLine = payload.line?.trim();
      if (!rawLine) {
        return;
      }
      const line = rawLine.replace(/^\[sensevoice\]\s*/i, "").trim() || rawLine;

      const entry = `[${payload.stream}] ${line}`;
      setLogLines((prev) => {
        const next = [...prev, entry];
        return next.slice(-100);
      });
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  return {
    status,
    dockerRuntimeStatus,
    progress,
    logLines,
    loading,
    refreshStatus,
    refreshDockerRuntimeStatus,
    prepare,
    updateSettings,
    startService,
    stopService,
    stopServiceForce,
    restartService,
    removeRuntimeContainer,
    updateRuntime,
  };
}
