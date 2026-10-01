import { useCallback, useEffect, useRef } from "react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { Settings } from "../types/settings";
import { useSenseVoice } from "./useSenseVoice";
import { toErrorMessage } from "../utils";

interface UseSenseVoiceManagementOptions {
  isSenseVoiceActive: boolean;
  draft: Settings | null;
}

export function useSenseVoiceManagement({
  isSenseVoiceActive,
  draft,
}: UseSenseVoiceManagementOptions) {
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
    removeModel: removeSenseVoiceModel,
  } = useSenseVoice(isSenseVoiceActive);

  const handleSenseVoicePrepare = useCallback(async () => {
    try {
      await prepareSenseVoice();
      toast.success(t("sensevoice.prepareQueued"));
      void refreshSenseVoiceStatus().catch(() => {});
    } catch (err) {
      toast.error(t("sensevoice.prepareError", { error: toErrorMessage(err) }));
    }
  }, [prepareSenseVoice, refreshSenseVoiceStatus, t]);

  const handleSenseVoiceStart = useCallback(async () => {
    try {
      await startSenseVoiceService();
      toast.success(t("sensevoice.startQueued"));
      void refreshSenseVoiceStatus().catch(() => {});
    } catch (err) {
      toast.error(t("sensevoice.startError", { error: toErrorMessage(err) }));
    }
  }, [startSenseVoiceService, refreshSenseVoiceStatus, t]);

  const handleSenseVoiceStop = useCallback(async () => {
    try {
      await stopSenseVoiceService();
      toast.success(t("sensevoice.stopSuccess"));
      void refreshSenseVoiceStatus().catch(() => {});
    } catch (err) {
      toast.error(t("sensevoice.stopError", { error: toErrorMessage(err) }));
    }
  }, [stopSenseVoiceService, refreshSenseVoiceStatus, t]);

  const handleSenseVoiceRemoveModel = useCallback(async () => {
    const ok = await ask(t("sensevoice.removeModelConfirm"), {
      title: t("sensevoice.removeModel"),
      kind: "warning",
    });
    if (!ok) {
      return;
    }
    try {
      await removeSenseVoiceModel();
      toast.success(t("sensevoice.removeModelSuccess"));
      void refreshSenseVoiceStatus().catch(() => {});
    } catch (err) {
      toast.error(t("sensevoice.removeModelError", { error: toErrorMessage(err) }));
    }
  }, [removeSenseVoiceModel, refreshSenseVoiceStatus, t]);

  const handleSenseVoiceConfigSave = useCallback(
    async (patch: {
      enabled?: boolean;
      installed?: boolean;
      localModel?: string;
      modelId?: string;
      language?: string;
    }) => {
      if (!draft) {
        return;
      }
      const next = {
        enabled: patch.enabled ?? draft.sensevoice.enabled,
        installed: patch.installed ?? draft.sensevoice.installed,
        localModel: patch.localModel ?? draft.sensevoice.localModel,
        modelId: patch.modelId ?? draft.sensevoice.modelId,
        language: patch.language ?? draft.sensevoice.language,
      };
      try {
        await updateSenseVoiceSettings({
          ...draft.sensevoice,
          ...next,
        });
        const status = await refreshSenseVoiceStatus();
        // 点击选择后下载：模型切换后若本地缺失则自动开始下载
        if (!status.installed && status.downloadState !== "downloading") {
          void handleSenseVoicePrepare();
        }
      } catch (err) {
        toast.error(t("sensevoice.configSaveError", { error: toErrorMessage(err) }));
      }
    },
    [
      draft,
      updateSenseVoiceSettings,
      refreshSenseVoiceStatus,
      handleSenseVoicePrepare,
      t,
    ]
  );

  // 本地模型已下载且启用时自动加载
  const pendingAutoStartRef = useRef(false);
  useEffect(() => {
    if (!isSenseVoiceActive || !sensevoiceStatus) {
      pendingAutoStartRef.current = false;
      return;
    }
    const installed = sensevoiceStatus.installed;
    const enabled = sensevoiceStatus.enabled;
    const ready = sensevoiceStatus.downloadState === "ready";
    const busy =
      sensevoiceStatus.runtimeState === "starting" ||
      sensevoiceStatus.downloadState === "downloading" ||
      sensevoiceStatus.downloadState === "loading";
    if (installed && enabled && ready && !sensevoiceStatus.running && !busy) {
      if (!pendingAutoStartRef.current) {
        pendingAutoStartRef.current = true;
        void handleSenseVoiceStart();
      }
    } else {
      pendingAutoStartRef.current = false;
    }
  }, [isSenseVoiceActive, sensevoiceStatus, handleSenseVoiceStart]);

  // 设置里启用本地转写但模型未下载时提示下载
  const autoPromptedForDownloadRef = useRef(false);
  useEffect(() => {
    const enabled = draft?.sensevoice.enabled;
    const installed = sensevoiceStatus.installed;
    const downloading = sensevoiceStatus.downloadState === "downloading";
    if (!(isSenseVoiceActive && enabled && !installed && !downloading)) {
      autoPromptedForDownloadRef.current = false;
      return;
    }
    if (autoPromptedForDownloadRef.current) {
      return;
    }
    autoPromptedForDownloadRef.current = true;
    void (async () => {
      const ok = await ask(t("sensevoice.startupDownloadPrompt"), {
        title: t("common.appName"),
        kind: "info",
      });
      if (!ok) {
        return;
      }
      await handleSenseVoicePrepare();
    })();
  }, [
    isSenseVoiceActive,
    draft?.sensevoice.enabled,
    sensevoiceStatus.installed,
    sensevoiceStatus.downloadState,
    handleSenseVoicePrepare,
    t,
  ]);

  return {
    sensevoiceStatus,
    sensevoiceProgress,
    sensevoiceLogLines,
    sensevoiceLoading,
    handleSenseVoicePrepare,
    handleSenseVoiceStart,
    handleSenseVoiceStop,
    handleSenseVoiceRemoveModel,
    handleSenseVoiceConfigSave,
    refreshSenseVoiceStatus,
  };
}
