import { useCallback } from "react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

export function useAutostart() {
  const isAutostartEnabled = useCallback(async () => {
    return await isEnabled();
  }, []);

  const syncAutostart = useCallback(async (enabled: boolean) => {
    const current = await isEnabled();
    if (current === enabled) {
      return current;
    }
    if (enabled) {
      await enable();
      return true;
    }
    await disable();
    return false;
  }, []);

  return {
    isAutostartEnabled,
    syncAutostart,
  };
}
