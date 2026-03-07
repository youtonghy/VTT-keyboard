import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import type { UpdateStatusPayload } from "../types/updater";

const DEFAULT_UPDATE_STATUS: UpdateStatusPayload = {
  status: "idle",
  currentVersion: "",
  latestVersion: null,
  notes: null,
  pubDate: null,
  downloadedBytes: null,
  totalBytes: null,
  error: null,
};

const toErrorMessage = (error: unknown) => {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};

export function useUpdater() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<UpdateStatusPayload>(DEFAULT_UPDATE_STATUS);

  useEffect(() => {
    let active = true;

    const loadStatus = async () => {
      try {
        const next = await invoke<UpdateStatusPayload>("get_update_status");
        if (active) {
          setStatus(next);
        }
      } catch (error) {
        toast.error(t("updater.loadError", { error: toErrorMessage(error) }));
      }
    };

    const unlisten = listen<UpdateStatusPayload>("update-status-changed", (event) => {
      if (active) {
        setStatus(event.payload);
      }
    });

    void loadStatus();

    return () => {
      active = false;
      void unlisten.then((dispose) => dispose());
    };
  }, [t]);

  const installUpdate = async () => {
    try {
      setStatus((prev) => ({ ...prev, status: "installing", error: null }));
      await invoke("install_downloaded_update");
    } catch (error) {
      toast.error(t("updater.installError", { error: toErrorMessage(error) }));
    }
  };

  const retryUpdateCheck = async () => {
    try {
      await invoke("retry_update_check");
    } catch (error) {
      toast.error(t("updater.retryError", { error: toErrorMessage(error) }));
    }
  };

  const dismissUpdateError = async () => {
    try {
      await invoke("dismiss_update_error");
    } catch (error) {
      toast.error(t("updater.dismissError", { error: toErrorMessage(error) }));
    }
  };

  return {
    status,
    installUpdate,
    retryUpdateCheck,
    dismissUpdateError,
  };
}