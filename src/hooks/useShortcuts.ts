import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { toast } from "sonner";
import { toErrorMessage } from "../utils";

const modifierKeys = new Set(["Shift", "Control", "Alt", "Meta"]);

const logDebug = (..._args: unknown[]) => {};
const logError = (..._args: unknown[]) => {};

const isConflictError = (message: string) => {
  const lowered = message.toLowerCase();
  return (
    lowered.includes("already") ||
    lowered.includes("registered") ||
    lowered.includes("conflict") ||
    lowered.includes("in use")
  );
};

const normalizeShortcutKey = (key: string) => {
  if (key === " ") {
    return "Space";
  }
  if (key.startsWith("Arrow")) {
    return key.replace("Arrow", "");
  }
  if (key.length === 1) {
    return key.toUpperCase();
  }
  return key;
};

const buildShortcut = (event: KeyboardEvent) => {
  const parts: string[] = [];
  if (event.metaKey || event.ctrlKey) {
    parts.push("CommandOrControl");
  }
  if (event.altKey) {
    parts.push("Alt");
  }
  if (event.shiftKey) {
    parts.push("Shift");
  }
  const key = normalizeShortcutKey(event.key);
  parts.push(key);
  return parts.join("+");
};

type BeforeStartRecording = () => boolean | Promise<boolean>;

export function useShortcuts(
  shortcutKey: string | undefined,
  onShortcutCaptured: (key: string) => void,
  beforeStartRecording?: BeforeStartRecording
) {
  const { t } = useTranslation();
  const tRef = useRef(t);
  useEffect(() => { tRef.current = t; }, [t]);
  const beforeStartRecordingRef = useRef(beforeStartRecording);
  useEffect(() => {
    beforeStartRecordingRef.current = beforeStartRecording;
  }, [beforeStartRecording]);

  const [isCapturing, setIsCapturing] = useState(false);

  useEffect(() => {
    if (!shortcutKey) {
      return;
    }
    let active = true;

    const LONG_PRESS_THRESHOLD_MS = 300;
    let isRecording = false;
    let pressStartTime: number | null = null;
    let keyDown = false;
    let inFlight = false;
    let startInFlight = false;
    let stopPending = false;

    const stopRecording = (silent = false) => {
      isRecording = false;
      pressStartTime = null;
      return invoke("stop_recording")
        .then(() => {
          stopPending = false;
          logDebug("stop_recording ok");
        })
        .catch((error) => {
          const message = toErrorMessage(error);
          logError("stop_recording failed", message);
          if (!silent) {
            toast.error(tRef.current("shortcut.stopError", { error: message }));
          }
          return releaseMicrophoneCapture();
        });
    };

    const releaseMicrophoneCapture = (silent = true) => {
      isRecording = false;
      pressStartTime = null;
      return invoke("abort_recording")
        .then(() => logDebug("abort_recording ok"))
        .catch((error) => {
          const message = toErrorMessage(error);
          logError("abort_recording failed", message);
          if (!silent) {
            toast.error(tRef.current("shortcut.stopError", { error: message }));
          }
        });
    };

    const doStart = async () => {
      if (inFlight) return;
      inFlight = true;
      startInFlight = true;
      stopPending = false;
      pressStartTime = Date.now();
      try {
        const canStart = await (beforeStartRecordingRef.current?.() ?? true);
        if (!canStart) {
          isRecording = false;
          pressStartTime = null;
          return;
        }

        await invoke("start_recording");
        logDebug("start_recording ok");
        isRecording = true;
        pressStartTime = Date.now();
        if (stopPending) {
          await stopRecording();
        }
      } catch (error) {
        isRecording = false;
        pressStartTime = null;
        const message = toErrorMessage(error);
        logError("start_recording failed", message);
        toast.error(tRef.current("shortcut.startError", { error: message }));
      } finally {
        startInFlight = false;
        inFlight = false;
      }
    };

    const doStop = (silent = false) => {
      stopPending = true;
      if (inFlight) return;
      inFlight = true;
      stopRecording(silent).finally(() => { inFlight = false; });
    };

    const registerShortcut = async () => {
      try {
        await unregisterAll();
        logDebug("unregister all success");
      } catch (error) {
        logError("unregister all failed", error);
        toast.error(tRef.current("shortcut.unregisterError"));
      }

      try {
        await register(shortcutKey, (event: { state: string }) => {
          if (!active) {
            return;
          }
          logDebug("event", event.state);

          if (event.state === "Pressed") {
            if (keyDown) return;
            keyDown = true;

            if (!isRecording) {
              void doStart();
            } else {
              doStop();
            }
          }

          if (event.state === "Released") {
            keyDown = false;

            if ((isRecording || startInFlight) && pressStartTime != null) {
              const duration = Date.now() - pressStartTime;
              if (duration >= LONG_PRESS_THRESHOLD_MS) {
                doStop();
              }
            }
          }
        });
        logDebug("register success", shortcutKey);
      } catch (error) {
        const message = toErrorMessage(error);
        logError("register failed", message);
        if (isConflictError(message)) {
          toast.error(tRef.current("shortcut.conflict", { shortcut: shortcutKey }));
        } else {
          toast.error(tRef.current("shortcut.registerError", { error: message }));
        }
      }
    };

    void registerShortcut();

    return () => {
      active = false;
      void releaseMicrophoneCapture();
      unregisterAll()
        .then(() => logDebug("unregister all cleanup"))
        .catch((error) => logError("unregister cleanup failed", error));
    };
  }, [shortcutKey]);

  useEffect(() => {
    if (!isCapturing) {
      return;
    }
    const handleKeydown = (event: KeyboardEvent) => {
      if (modifierKeys.has(event.key)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const hasModifier = event.ctrlKey || event.metaKey || event.altKey || event.shiftKey;
      if (!hasModifier) {
        toast.error(t("shortcut.requireModifier"));
        setIsCapturing(false);
        return;
      }
      const shortcut = buildShortcut(event);
      onShortcutCaptured(shortcut);
      setIsCapturing(false);
      toast.success(t("shortcut.captureSuccess", { shortcut }));
    };
    window.addEventListener("keydown", handleKeydown, true);
    return () => window.removeEventListener("keydown", handleKeydown, true);
  }, [isCapturing, t, onShortcutCaptured]);

  return { isCapturing, setIsCapturing };
}
