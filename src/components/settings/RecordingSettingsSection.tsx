import { RefreshCw, Square, Volume2 } from "lucide-react";
import { Button } from "@/components/motion/button/base";
import { cn } from "@/lib/utils";
import type { TFunction } from "i18next";
import type { Settings } from "../../types/settings";
import type {
  AudioInputDevice,
  AudioInputTestResult,
  MicrophonePermissionStatus,
} from "../../hooks/useAudioInputDevices";
import { CustomSelect } from "../CustomSelect";
import { Field } from "../FormFields";
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

  const deviceOptions = [
    {
      value: SYSTEM_INPUT_DEVICE,
      label: t("recording.systemAuto"),
      description: defaultDevice?.name,
    },
    ...devices.map((device) => ({
      value: device.name,
      label: device.name,
      description: device.isDefault ? t("recording.defaultDevice") : undefined,
    })),
  ];

  return (
    <>
      <Field label={t("recording.segmentSeconds")}>
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
      </Field>

      <div className="grid gap-4 rounded-2xl border border-border bg-muted/30 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h3 className="font-medium text-sm">{t("recording.inputDevice")}</h3>
            <p className="mt-1 text-muted-foreground text-xs leading-5">
              {selectedDevice
                ? t("recording.selectedDeviceHint", { device: selectedDevice.name })
                : defaultDevice
                  ? t("recording.systemDeviceHint", { device: defaultDevice.name })
                  : t("recording.systemDeviceFallbackHint")}
            </p>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="shrink-0 rounded-full"
            disabled={devicesLoading}
            onClick={onRefreshDevices}
            aria-label={t("recording.refreshDevices")}
          >
            <RefreshCw size={16} className={cn(devicesLoading && "animate-spin")} />
          </Button>
        </div>

        <CustomSelect
          ariaLabel={t("recording.inputDevice")}
          value={selectedInputDeviceName}
          options={deviceOptions}
          onChange={(value) =>
            updateDraft((prev) => ({
              ...prev,
              recording: { ...prev.recording, inputDeviceName: value },
            }))
          }
        />

        {devicesError ? <p className="text-destructive text-xs">{devicesError}</p> : null}
        {permissionTranslationKey ? (
          <p className="text-warning text-xs">{t(permissionTranslationKey)}</p>
        ) : null}

        <div className="flex flex-wrap items-center gap-4">
          <Button
            variant={inputTestActive ? "danger" : "secondary"}
            onClick={onTestInput}
          >
            {inputTestActive ? <Square size={16} /> : <Volume2 size={16} />}
            {inputTestActive ? t("recording.stopTestingInput") : t("recording.testInput")}
          </Button>
          <div className="grid min-w-48 flex-1 gap-1.5">
            <div className="flex justify-between text-muted-foreground text-xs">
              <span>{t("recording.inputLevel")}</span>
              <span className="tabular-nums">{level}%</span>
            </div>
            <div
              role="progressbar"
              aria-label={t("recording.inputLevel")}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={level}
              className="h-2 overflow-hidden rounded-full bg-muted"
            >
              <div
                className="h-full rounded-full bg-accent transition-[width] duration-100 ease-out"
                style={{ width: `${level}%` }}
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
