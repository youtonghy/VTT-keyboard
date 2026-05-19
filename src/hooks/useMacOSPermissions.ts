import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  unsupportedMacOSPermissionStatus,
  type MacOSPermissionId,
  type MacOSPermissionStatus,
} from "../utils/permissions";

export {
  isMacOSPermissionApproved,
  type MacOSPermissionId,
  type MacOSPermissionItem,
  type MacOSPermissionStatus,
} from "../utils/permissions";

export function useMacOSPermissions(enabled: boolean) {
  const [permissions, setPermissions] = useState<MacOSPermissionStatus>(
    unsupportedMacOSPermissionStatus
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refreshPermissions = useCallback(async () => {
    if (!enabled) {
      setPermissions(unsupportedMacOSPermissionStatus);
      return unsupportedMacOSPermissionStatus;
    }

    setLoading(true);
    setError("");
    try {
      const next = await invoke<MacOSPermissionStatus>("get_macos_permission_status");
      setPermissions(next);
      return next;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      throw err;
    } finally {
      setLoading(false);
    }
  }, [enabled]);

  const requestPermission = useCallback(
    async (permissionId: MacOSPermissionId) => {
      if (!enabled) {
        setPermissions(unsupportedMacOSPermissionStatus);
        return unsupportedMacOSPermissionStatus;
      }

      setLoading(true);
      setError("");
      try {
        const next = await invoke<MacOSPermissionStatus>("request_macos_permission", {
          permissionId,
        });
        setPermissions(next);
        return next;
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        throw err;
      } finally {
        setLoading(false);
      }
    },
    [enabled]
  );

  useEffect(() => {
    if (!enabled) {
      setPermissions(unsupportedMacOSPermissionStatus);
      return;
    }

    void refreshPermissions();
  }, [enabled, refreshPermissions]);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handleFocus = () => {
      void refreshPermissions();
    };

    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, [enabled, refreshPermissions]);

  useEffect(() => {
    if (!enabled || typeof window.__TAURI_INTERNALS__?.invoke !== "function") {
      return;
    }

    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) {
        void refreshPermissions();
      }
    });

    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, [enabled, refreshPermissions]);

  return {
    error,
    loading,
    permissions,
    refreshPermissions,
    requestPermission,
  };
}
