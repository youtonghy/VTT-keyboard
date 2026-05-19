import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type MacOSPermissionId = "microphone" | "accessibility";

export interface MacOSPermissionItem {
  id: MacOSPermissionId;
  status: string;
  required: boolean;
}

export interface MacOSPermissionStatus {
  supported: boolean;
  microphone: MacOSPermissionItem;
  accessibility: MacOSPermissionItem;
}

const unsupportedPermissionStatus: MacOSPermissionStatus = {
  supported: false,
  microphone: {
    id: "microphone",
    status: "unsupported",
    required: false,
  },
  accessibility: {
    id: "accessibility",
    status: "unsupported",
    required: false,
  },
};

export const isMacOSPermissionApproved = (status: string) =>
  status === "authorized" || status === "approved";

export function useMacOSPermissions(enabled: boolean) {
  const [permissions, setPermissions] = useState<MacOSPermissionStatus>(
    unsupportedPermissionStatus
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refreshPermissions = useCallback(async () => {
    if (!enabled) {
      setPermissions(unsupportedPermissionStatus);
      return unsupportedPermissionStatus;
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
        setPermissions(unsupportedPermissionStatus);
        return unsupportedPermissionStatus;
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
      setPermissions(unsupportedPermissionStatus);
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

  return {
    error,
    loading,
    permissions,
    refreshPermissions,
    requestPermission,
  };
}
