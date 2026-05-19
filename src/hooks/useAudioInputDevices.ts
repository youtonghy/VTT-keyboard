import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AudioInputDevice {
  name: string;
  isDefault: boolean;
}

export interface AudioInputTestResult {
  peakLevel: number;
  averageLevel: number;
  sampleCount: number;
  sampleRate: number;
  channels: number;
}

export function useAudioInputDevices(isActive: boolean) {
  const [devices, setDevices] = useState<AudioInputDevice[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refreshDevices = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const nextDevices = await invoke<AudioInputDevice[]>("list_audio_input_devices");
      setDevices(nextDevices);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setDevices([]);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const testDevice = useCallback(async (inputDeviceName: string) => {
    return invoke<AudioInputTestResult>("test_audio_input_device", {
      inputDeviceName,
    });
  }, []);

  useEffect(() => {
    if (isActive) {
      void refreshDevices();
    }
  }, [isActive, refreshDevices]);

  return {
    devices,
    error,
    loading,
    refreshDevices,
    testDevice,
  };
}
