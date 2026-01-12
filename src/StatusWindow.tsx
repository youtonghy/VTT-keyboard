import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useTranslation } from "react-i18next";
import "./App.css";

type StatusState = "recording" | "transcribing" | "completed" | "error";

export default function StatusWindow() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<StatusState | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("status-mode");
    return () => document.documentElement.classList.remove("status-mode");
  }, []);

  useEffect(() => {
    const appWindow = getCurrentWebviewWindow();
    let timer: number | null = null;
    const stopListening = listen<string>("status-update", (event) => {
      const nextStatus = event.payload as StatusState;
      setStatus(nextStatus);
      if (import.meta.env.DEV) {
        console.log("[status]", nextStatus);
      }
      if (timer) {
        window.clearTimeout(timer);
        timer = null;
      }
      appWindow.show();
      if (nextStatus === "completed" || nextStatus === "error") {
        timer = window.setTimeout(() => {
          appWindow.hide();
          setStatus(null);
        }, 2000);
      }
    });
    stopListening.catch((error) => {
      if (import.meta.env.DEV) {
        console.error("[status] listen failed", error);
      }
    });

    return () => {
      if (timer) {
        window.clearTimeout(timer);
      }
      void stopListening.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <div className="status-page">
      {status ? (
        <div className={`status-window ${status}`}>
          <span>{t(`status.${status}`)}</span>
        </div>
      ) : null}
    </div>
  );
}
