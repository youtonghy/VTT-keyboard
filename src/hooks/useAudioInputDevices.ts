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
  isSilent: boolean;
}

export interface MicrophonePermissionStatus {
  status: string;
  supported: boolean;
}

export function useAudioInputDevices(isActive: boolean) {
  const [devices, setDevices] = useState<AudioInputDevice[]>([]);
  const [permissionStatus, setPermissionStatus] = useState<MicrophonePermissionStatus>({
    status: "unknown",
    supported: false,
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refreshDevices = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const [nextPermissionStatus, nextDevices] = await Promise.all([
        invoke<MicrophonePermissionStatus>("get_microphone_permission_status"),
        invoke<AudioInputDevice[]>("list_audio_input_devices"),
      ]);
      setPermissionStatus(nextPermissionStatus);
      setDevices(nextDevices);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setDevices([]);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const requestMicrophonePermission = useCallback(async () => {
    const nextPermissionStatus = await invoke<MicrophonePermissionStatus>(
      "request_microphone_permission"
    );
    setPermissionStatus(nextPermissionStatus);
    return nextPermissionStatus;
  }, []);

  const testDevice = useCallback(async (inputDeviceName: string, durationMs = 180) => {
    await requestMicrophonePermission();
    return invoke<AudioInputTestResult>("test_audio_input_device", {
      inputDeviceName,
      durationMs,
    });
  }, [requestMicrophonePermission]);

  useEffect(() => {
    if (isActive) {
      void refreshDevices();
    }
  }, [isActive, refreshDevices]);

  return {
    devices,
    error,
    loading,
    permissionStatus,
    requestMicrophonePermission,
    refreshDevices,
    testDevice,
  };
}
