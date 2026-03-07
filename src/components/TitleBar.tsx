import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { UpdateStatusPayload } from "../types/updater";

interface TitleBarProps {
  updateStatus?: UpdateStatusPayload;
  onInstallUpdate?: () => void;
  onRetryUpdateCheck?: () => void;
  onDismissUpdateError?: () => void;
}

const formatProgress = (status: UpdateStatusPayload, downloadingLabel: string) => {
  if (status.status !== "downloading") {
    return null;
  }
  if (!status.totalBytes || !status.downloadedBytes) {
    return downloadingLabel;
  }
  const percent = Math.max(
    0,
    Math.min(100, Math.round((status.downloadedBytes / status.totalBytes) * 100))
  );
  return `${downloadingLabel} ${percent}%`;
};

export function TitleBar({
  updateStatus,
  onInstallUpdate,
  onRetryUpdateCheck,
  onDismissUpdateError,
}: TitleBarProps) {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);
  const appWindow = useMemo(() => getCurrentWindow(), []);

  useEffect(() => {
    const updateState = async () => {
      setIsMaximized(await appWindow.isMaximized());
    };

    void updateState();
    const unlisten = appWindow.listen("tauri://resize", updateState);
    return () => {
      void unlisten.then((f) => f());
    };
  }, [appWindow]);

  const minimize = () => appWindow.minimize();
  const toggleMaximize = async () => {
    await appWindow.toggleMaximize();
    setIsMaximized((prev) => !prev);
  };
  const close = () => appWindow.close();

  const latestVersion = updateStatus?.latestVersion ?? "";
  const showUpdateState =
    updateStatus &&
    ["checking", "available", "downloading", "downloaded", "installing", "error"].includes(
      updateStatus.status
    );
  const progressText = updateStatus
    ? formatProgress(updateStatus, t("updater.downloading"))
    : null;

  return (
    <div className="titlebar">
      <div className="titlebar-drag-region" data-tauri-drag-region>
        <div className="titlebar-title">VTT Keyboard</div>
      </div>
      <div className="titlebar-controls">
        {showUpdateState ? (
          <div className={`titlebar-update titlebar-update-${updateStatus?.status ?? "idle"}`}>
            {updateStatus?.status === "checking" ? (
              <span className="titlebar-update-chip">{t("updater.checking")}</span>
            ) : null}
            {updateStatus?.status === "available" ? (
              <span className="titlebar-update-chip">
                {t("updater.availableShort", { version: latestVersion })}
              </span>
            ) : null}
            {updateStatus?.status === "downloading" ? (
              <span className="titlebar-update-chip">{progressText}</span>
            ) : null}
            {updateStatus?.status === "downloaded" ? (
              <button
                type="button"
                className="titlebar-update-action"
                onClick={onInstallUpdate}
                title={t("updater.installNow")}
              >
                {t("updater.installNowShort", { version: latestVersion })}
              </button>
            ) : null}
            {updateStatus?.status === "installing" ? (
              <span className="titlebar-update-chip">{t("updater.installing")}</span>
            ) : null}
            {updateStatus?.status === "error" ? (
              <>
                <span className="titlebar-update-chip error" title={updateStatus.error ?? undefined}>
                  {t("updater.error")}
                </span>
                <button
                  type="button"
                  className="titlebar-update-action"
                  onClick={onRetryUpdateCheck}
                  title={t("updater.retry")}
                >
                  {t("updater.retry")}
                </button>
                <button
                  type="button"
                  className="titlebar-update-dismiss"
                  onClick={onDismissUpdateError}
                  title={t("updater.dismiss")}
                >
                  x
                </button>
              </>
            ) : null}
          </div>
        ) : null}
        <button className="titlebar-button" onClick={minimize} title={t("titleBar.minimize")}>
          <svg width="10" height="1" viewBox="0 0 10 1">
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          className="titlebar-button"
          onClick={toggleMaximize}
          title={isMaximized ? t("titleBar.restore") : t("titleBar.maximize")}
        >
          {isMaximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path
                d="M2,0 L10,0 L10,8 L8,8 L8,10 L0,10 L0,2 L2,2 L2,0 Z M8,2 L8,8 L2,8 L2,2 L8,2 Z"
                fill="currentColor"
                fillRule="evenodd"
              />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <rect width="10" height="10" stroke="currentColor" strokeWidth="1" fill="none" />
            </svg>
          )}
        </button>
        <button className="titlebar-button close" onClick={close} title={t("titleBar.close")}>
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M0,0 L10,10 M10,0 L0,10" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>
    </div>
  );
}