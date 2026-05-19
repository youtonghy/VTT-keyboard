import { Button, Label, ListBox, ProgressBar, Select } from "@heroui/react";
import { Check, RefreshCw, Square, Volume2 } from "lucide-react";
import type { TFunction } from "i18next";
import type { Settings } from "../../types/settings";
import type {
  AudioInputDevice,
  AudioInputTestResult,
  MicrophonePermissionStatus,
} from "../../hooks/useAudioInputDevices";
import { HeroField } from "../HeroField";
import { NumberWheelInput } from "../NumberWheelInput";

interface RecordingSettingsSectionProps {
  draft: Settings;
  devices: AudioInputDevice[];
  devicesError: string;
  devicesLoading: boolean;
  microphonePermissionStatus: MicrophonePermissionStatus;
  inputTestResult: AudioInputTestResult | null;
  inputTestActive: boolean;
  t: TFunction;
  onRefreshDevices: () => void;
  onTestInput: () => void;
  updateDraft: (updater: (previous: Settings) => Settings) => void;
}

const SYSTEM_INPUT_DEVICE = "";

export function RecordingSettingsSection({
  draft,
  devices,
  devicesError,
  devicesLoading,
  microphonePermissionStatus,
  inputTestResult,
  inputTestActive,
  t,
  onRefreshDevices,
  onTestInput,
  updateDraft,
}: RecordingSettingsSectionProps) {
  const selectedInputDeviceName = draft.recording.inputDeviceName ?? SYSTEM_INPUT_DEVICE;
  const selectedDevice = devices.find((device) => device.name === selectedInputDeviceName);
  const defaultDevice = devices.find((device) => device.isDefault);
  const level = Math.round((inputTestResult?.peakLevel ?? 0) * 100);
  const permissionTranslationKey =
    microphonePermissionStatus.supported &&
    microphonePermissionStatus.status !== "authorized"
      ? `recording.microphonePermission.${microphonePermissionStatus.status}`
      : "";

  return (
    <>
      <HeroField label={t("recording.segmentSeconds")}>
        <NumberWheelInput
          min={10}
          value={draft.recording.segmentSeconds}
          onChange={(value) =>
            updateDraft((prev) => ({
              ...prev,
              recording: { ...prev.recording, segmentSeconds: value },
            }))
          }
        />
      </HeroField>

      <div className="recording-device-panel">
        <div className="recording-device-header">
          <div>
            <h4>{t("recording.inputDevice")}</h4>
            <p>
              {selectedDevice
                ? t("recording.selectedDeviceHint", { device: selectedDevice.name })
                : defaultDevice
                  ? t("recording.systemDeviceHint", { device: defaultDevice.name })
                  : t("recording.systemDeviceFallbackHint")}
            </p>
          </div>
          <Button
            type="button"
            variant="tertiary"
            isIconOnly
            isPending={devicesLoading}
            onPress={onRefreshDevices}
            aria-label={t("recording.refreshDevices")}
          >
            <RefreshCw size={16} />
          </Button>
        </div>

        <Select
          aria-label={t("recording.inputDevice")}
          selectedKey={selectedInputDeviceName}
          onSelectionChange={(key) => {
            if (key == null) {
              return;
            }
            updateDraft((prev) => ({
              ...prev,
              recording: {
                ...prev.recording,
                inputDeviceName: String(key),
              },
            }));
          }}
        >
          <Select.Trigger className="hero-select-trigger">
            <Select.Value>
              {selectedDevice?.name ?? t("recording.systemAuto")}
            </Select.Value>
            <Select.Indicator />
          </Select.Trigger>
          <Select.Popover className="hero-select-popover">
            <ListBox>
              <ListBox.Item id={SYSTEM_INPUT_DEVICE} textValue={t("recording.systemAuto")}>
                <span className="recording-device-option">
                  <span>{t("recording.systemAuto")}</span>
                  {defaultDevice ? <small>{defaultDevice.name}</small> : null}
                </span>
                <ListBox.ItemIndicator>
                  <Check size={14} />
                </ListBox.ItemIndicator>
              </ListBox.Item>
              {devices.map((device) => (
                <ListBox.Item key={device.name} id={device.name} textValue={device.name}>
                  <span className="recording-device-option">
                    <span>{device.name}</span>
                    {device.isDefault ? <small>{t("recording.defaultDevice")}</small> : null}
                  </span>
                  <ListBox.ItemIndicator>
                    <Check size={14} />
                  </ListBox.ItemIndicator>
                </ListBox.Item>
              ))}
            </ListBox>
          </Select.Popover>
        </Select>

        {devicesError ? <p className="recording-device-error">{devicesError}</p> : null}
        {permissionTranslationKey ? (
          <p className="recording-permission-status">{t(permissionTranslationKey)}</p>
        ) : null}

        <div className="recording-test-row">
          <Button
            type="button"
            variant={inputTestActive ? "danger-soft" : "secondary"}
            onPress={onTestInput}
          >
            {inputTestActive ? <Square size={16} /> : <Volume2 size={16} />}
            {inputTestActive ? t("recording.stopTestingInput") : t("recording.testInput")}
          </Button>
          <ProgressBar
            aria-label={t("recording.inputLevel")}
            className="recording-level"
            value={level}
            maxValue={100}
          >
            <Label>{t("recording.inputLevel")}</Label>
            <ProgressBar.Output>{level}%</ProgressBar.Output>
            <ProgressBar.Track>
              <ProgressBar.Fill />
            </ProgressBar.Track>
          </ProgressBar>
        </div>
      </div>
    </>
  );
}
