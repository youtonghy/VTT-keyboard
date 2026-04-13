import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import type { Settings } from "../types/settings";
import { normalizeAliyunRegion, toErrorMessage } from "../utils";
import {
  normalizeLocalModel,
  normalizeStopMode,
  normalizeSenseVoiceModelId,
  normalizeSenseVoiceLanguage,
  normalizeSenseVoiceDevice,
} from "../utils/sensevoice";

interface UseSettingsSyncParams {
  settings: Settings | null;
  setSettings: (s: Settings) => void;
  saveSettings: (s: Settings) => Promise<Settings>;
  syncAutostart: (enabled: boolean) => Promise<boolean>;
  supportsSherpaOnnxSenseVoice: boolean;
}

export function useSettingsSync({
  settings,
  setSettings,
  saveSettings,
  syncAutostart,
  supportsSherpaOnnxSenseVoice,
}: UseSettingsSyncParams) {
  const { t } = useTranslation();
  const [draft, setDraftState] = useState<Settings | null>(null);
  const skipNextDraftSync = useRef(false);
  const autostartSyncedOnStartup = useRef(false);

  const createDraftFromSettings = useCallback(
    (s: Settings): Settings => {
      const normalizedLocalModel = normalizeLocalModel(s.sensevoice.localModel);
      const effectiveLocalModel =
        !supportsSherpaOnnxSenseVoice &&
        normalizedLocalModel === "sherpa-onnx-sensevoice"
          ? "sensevoice"
          : normalizedLocalModel;
      const normalizedAliyunRegion = normalizeAliyunRegion(s.aliyun.region);
      const effectiveAliyunRegion =
        s.provider === "aliyun-paraformer" ? "beijing" : normalizedAliyunRegion;
      return {
        ...s,
        sensevoice: {
          ...s.sensevoice,
          localModel: effectiveLocalModel,
          stopMode: normalizeStopMode(s.sensevoice.stopMode),
          modelId: normalizeSenseVoiceModelId(effectiveLocalModel, s.sensevoice.modelId),
          language: normalizeSenseVoiceLanguage(s.sensevoice.language),
          device: normalizeSenseVoiceDevice(effectiveLocalModel, s.sensevoice.device),
        },
        aliyun: {
          ...s.aliyun,
          region: effectiveAliyunRegion,
          apiKeys: {
            beijing: s.aliyun.apiKeys.beijing ?? "",
            singapore: s.aliyun.apiKeys.singapore ?? "",
          },
          asr: {
            ...s.aliyun.asr,
            vocabularyId: s.aliyun.asr.vocabularyId ?? "",
          },
          paraformer: {
            ...s.aliyun.paraformer,
            vocabularyId: s.aliyun.paraformer.vocabularyId ?? "",
            languageHints: s.aliyun.paraformer.languageHints ?? [],
          },
        },
      };
    },
    [supportsSherpaOnnxSenseVoice]
  );

  useEffect(() => {
    if (!settings) return;
    if (skipNextDraftSync.current) {
      skipNextDraftSync.current = false;
      return;
    }
    setDraftState(createDraftFromSettings(settings));
  }, [settings, createDraftFromSettings]);

  useEffect(() => {
    if (!settings || autostartSyncedOnStartup.current) {
      return;
    }
    autostartSyncedOnStartup.current = true;
    void syncAutostart(settings.startup.launchOnBoot).catch((error) => {
      toast.error(t("general.launchOnBootSyncError", { error: toErrorMessage(error) }));
    });
  }, [settings, syncAutostart, t]);

  const updateDraft = (updater: (prev: Settings) => Settings) => {
    setDraftState((prev) => (prev ? updater(prev) : prev));
  };

  const buildPersistedSettings = useCallback(() => {
    if (!draft) {
      return null;
    }
    return { ...draft };
  }, [draft]);

  const validateTriggers = (next: Settings) => {
    const invalidRange = next.triggers.find((card) =>
      card.variables.every((value) => value.trim().length === 0)
    );
    if (invalidRange) {
      return t("triggers.validationVariables", { title: invalidRange.title });
    }
    const invalidKeyword = next.triggers.find((card) => card.keyword.trim().length === 0);
    if (invalidKeyword) {
      return t("triggers.validationKeyword", { title: invalidKeyword.title });
    }
    const invalidKeywordPlaceholder = next.triggers.find((card) => {
      const count = card.keyword.split("{value}").length - 1;
      return count > 1;
    });
    if (invalidKeywordPlaceholder) {
      return t("triggers.validationKeywordPlaceholder", {
        title: invalidKeywordPlaceholder.title,
      });
    }
    return null;
  };

  const handleSave = async () => {
    const nextSettings = buildPersistedSettings();
    if (!nextSettings) {
      return;
    }
    const error = validateTriggers(nextSettings);
    if (error) {
      toast.error(error);
      return;
    }
    try {
      const persisted = await saveSettings(nextSettings);
      skipNextDraftSync.current = true;
      setDraftState((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          sensevoice: {
            ...prev.sensevoice,
            enabled: persisted.sensevoice.enabled,
            installed: persisted.sensevoice.installed,
            downloadState: persisted.sensevoice.downloadState,
            lastError: persisted.sensevoice.lastError,
          },
        };
      });
      try {
        await syncAutostart(persisted.startup.launchOnBoot);
      } catch (error) {
        toast.error(t("general.launchOnBootSyncError", { error: toErrorMessage(error) }));
        return;
      }
      toast.success(t("actions.saveSuccess"));
    } catch (err) {
      toast.error(t("actions.saveError") + ": " + toErrorMessage(err));
    }
  };

  const handleSaveRef = useRef(handleSave);
  handleSaveRef.current = handleSave;

  useEffect(() => {
    if (!draft || !settings) {
      return;
    }
    if (JSON.stringify(draft) === JSON.stringify(settings)) {
      return;
    }

    const timer = setTimeout(() => {
      handleSaveRef.current();
    }, 1000);

    return () => clearTimeout(timer);
  }, [draft, settings]);

  const handleImport = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const selected = Array.isArray(path) ? path[0] : path;
    if (!selected) {
      return;
    }
    try {
      const data = await invoke<Settings>("import_settings", { path: selected });
      setSettings(data);
      toast.success(t("data.importSuccess"));
    } catch (err) {
      toast.error(t("data.importError"));
    }
  };

  const handleExport = async () => {
    const path = await save({
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      return;
    }
    try {
      await invoke("export_settings", { path });
      toast.success(t("data.exportSuccess"));
    } catch (err) {
      toast.error(t("data.exportError"));
    }
  };

  return { draft, updateDraft, handleImport, handleExport };
}
