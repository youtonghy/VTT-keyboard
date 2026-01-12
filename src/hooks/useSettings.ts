import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../types/settings";

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);

  const loadSettings = useCallback(async () => {
    const data = await invoke<Settings>("get_settings");
    setSettings(data);
  }, []);

  const saveSettings = useCallback(async (next: Settings) => {
    await invoke("update_settings", { settings: next });
    setSettings(next);
  }, []);

  useEffect(() => {
    loadSettings().finally(() => setLoading(false));
  }, [loadSettings]);

  return {
    settings,
    setSettings,
    loading,
    saveSettings,
    reload: loadSettings,
  };
}
