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

  const saveSettings = useCallback(async (next: Settings): Promise<Settings> => {
    // update_settings returns the normalized/persisted version so React
    // state stays in sync with what's actually on disk.
    const persisted = await invoke<Settings>("update_settings", { settings: next });
    setSettings(persisted);
    return persisted;
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
