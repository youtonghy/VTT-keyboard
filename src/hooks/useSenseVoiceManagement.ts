import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useSenseVoice } from "./useSenseVoice";
import type { Settings } from "../types/settings";
import { toErrorMessage } from "../utils";
import { normalizeLocalModel, normalizeStopMode } from "../utils/sensevoice";

interface UseSenseVoiceManagementParams {
  isSenseVoiceActive: boolean;
  draft: Settings | null;
  supportsSherpaOnnxSenseVoice: boolean;
}

export function useSenseVoiceManagement({
  isSenseVoiceActive,
  draft,
  supportsSherpaOnnxSenseVoice,
}: UseSenseVoiceManagementParams) {
  const { t } = useTranslation();

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

  const [pendingSherpaAutoStart, setPendingSherpaAutoStart] = useState(false);

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
    if (!isSenseVoiceActive) {
      return;
    }
    void refreshSenseVoiceStatus().catch(() => {});
  }, [isSenseVoiceActive, refreshSenseVoiceStatus]);

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

  return {
    sensevoiceStatus,
    sensevoiceProgress,
    sensevoiceLogLines,
    sensevoiceLoading,
    refreshSenseVoiceStatus,
    handleSenseVoicePrepare,
    handleSenseVoiceStart,
    handleSenseVoiceStop,
  };
}
