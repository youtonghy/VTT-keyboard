import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useSenseVoice } from "./useSenseVoice";
import type { Settings } from "../types/settings";
import { toErrorMessage } from "../utils";
import { normalizeLocalModel } from "../utils/sensevoice";

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
    dockerRuntimeStatus: sensevoiceDockerRuntimeStatus,
    progress: sensevoiceProgress,
    logLines: sensevoiceLogLines,
    loading: sensevoiceLoading,
    refreshStatus: refreshSenseVoiceStatus,
    refreshDockerRuntimeStatus: refreshSenseVoiceDockerRuntimeStatus,
    prepare: prepareSenseVoice,
    updateSettings: updateSenseVoiceSettings,
    startService: startSenseVoiceService,
    stopService: stopSenseVoiceService,
    stopServiceForce: stopSenseVoiceServiceForce,
    restartService: restartSenseVoiceService,
    removeRuntimeContainer: removeSenseVoiceRuntimeContainer,
    updateRuntime: updateSenseVoiceRuntime,
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
      await Promise.all([
        refreshSenseVoiceStatus(),
        refreshSenseVoiceDockerRuntimeStatus(),
      ]);
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
      await Promise.all([
        refreshSenseVoiceStatus(),
        refreshSenseVoiceDockerRuntimeStatus(),
      ]);
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
      if (draft.sensevoice.stopMode === "pause") {
        await stopSenseVoiceService();
      } else {
        await stopSenseVoiceServiceForce();
      }
      await Promise.all([
        refreshSenseVoiceStatus(),
        refreshSenseVoiceDockerRuntimeStatus(),
      ]);
      const runtimeKind = sensevoiceStatus.runtimeKind;
      if (runtimeKind === "native") {
        toast.success(t("sensevoice.unloadSuccess"));
      } else if (draft.sensevoice.stopMode === "pause") {
        toast.success(t("sensevoice.pauseSuccess"));
      } else {
        toast.success(t("sensevoice.stopSuccess"));
      }
    } catch (error) {
      toast.error(t("sensevoice.stopError", { error: toErrorMessage(error) }));
    }
  };

  const handleSenseVoiceRestart = async () => {
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
      await restartSenseVoiceService();
      await Promise.all([
        refreshSenseVoiceStatus(),
        refreshSenseVoiceDockerRuntimeStatus(),
      ]);
      toast.success(t("sensevoice.restartQueued"));
    } catch (error) {
      toast.error(t("sensevoice.restartError", { error: toErrorMessage(error) }));
    }
  };

  const handleSenseVoiceRemoveContainer = async () => {
    const nextSenseVoiceSettings = buildPersistedSenseVoiceSettings();
    if (!nextSenseVoiceSettings) {
      return;
    }
    const confirmed = window.confirm(t("sensevoice.removeRuntimeConfirm"));
    if (!confirmed) {
      return;
    }
    try {
      await updateSenseVoiceSettings(nextSenseVoiceSettings);
    } catch (error) {
      toast.error(t("sensevoice.configSaveError", { error: toErrorMessage(error) }));
      return;
    }
    try {
      await removeSenseVoiceRuntimeContainer();
      await Promise.all([
        refreshSenseVoiceStatus(),
        refreshSenseVoiceDockerRuntimeStatus(),
      ]);
      toast.success(t("sensevoice.removeRuntimeSuccess"));
    } catch (error) {
      toast.error(t("sensevoice.removeRuntimeError", { error: toErrorMessage(error) }));
    }
  };

  const handleUpdateRuntime = async () => {
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
      await updateSenseVoiceRuntime();
      await Promise.all([
        refreshSenseVoiceStatus(),
        refreshSenseVoiceDockerRuntimeStatus(),
      ]);
      toast.success(t("sensevoice.updateRuntimeQueued"));
    } catch (error) {
      toast.error(t("sensevoice.updateRuntimeError", { error: toErrorMessage(error) }));
    }
  };

  useEffect(() => {
    if (!isSenseVoiceActive) {
      return;
    }
    void Promise.all([
      refreshSenseVoiceStatus(),
      refreshSenseVoiceDockerRuntimeStatus(),
    ]).catch(() => {});
  }, [isSenseVoiceActive, refreshSenseVoiceDockerRuntimeStatus, refreshSenseVoiceStatus]);

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
    sensevoiceDockerRuntimeStatus,
    sensevoiceProgress,
    sensevoiceLogLines,
    sensevoiceLoading,
    refreshSenseVoiceStatus,
    refreshSenseVoiceDockerRuntimeStatus,
    handleSenseVoicePrepare,
    handleSenseVoiceStart,
    handleSenseVoiceStop,
    handleSenseVoiceRestart,
    handleSenseVoiceRemoveContainer,
    handleUpdateRuntime,
  };
}
