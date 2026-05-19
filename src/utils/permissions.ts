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

export const unsupportedMacOSPermissionStatus: MacOSPermissionStatus = {
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

export const canStartMicrophoneTest = (isMacOS: boolean, microphoneStatus: string) =>
  !isMacOS || isMacOSPermissionApproved(microphoneStatus);
