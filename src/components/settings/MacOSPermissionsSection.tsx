import { Alert, Button, Chip } from "@heroui/react";
import { CheckCircle2, Keyboard, Mic, RefreshCw, ShieldAlert } from "lucide-react";
import type { TFunction } from "i18next";
import type {
  MacOSPermissionId,
  MacOSPermissionItem,
} from "../../hooks/useMacOSPermissions";
import { isMacOSPermissionApproved } from "../../hooks/useMacOSPermissions";

interface MacOSPermissionsSectionProps {
  microphone: MacOSPermissionItem;
  accessibility: MacOSPermissionItem;
  loading: boolean;
  error: string;
  t: TFunction;
  onRefresh: () => void;
  onRequestPermission: (permissionId: MacOSPermissionId) => void;
}

const permissionItems = [
  {
    id: "microphone" as const,
    Icon: Mic,
  },
  {
    id: "accessibility" as const,
    Icon: Keyboard,
  },
];

export function MacOSPermissionsSection({
  microphone,
  accessibility,
  loading,
  error,
  t,
  onRefresh,
  onRequestPermission,
}: MacOSPermissionsSectionProps) {
  const permissions = {
    microphone,
    accessibility,
  };
  const hasMissingPermission = permissionItems.some(({ id }) => {
    const item = permissions[id];
    return item.required && !isMacOSPermissionApproved(item.status);
  });
  const accessibilityMissing =
    accessibility.required && !isMacOSPermissionApproved(accessibility.status);

  return (
    <div className="macos-permissions-panel">
      {hasMissingPermission ? (
        <Alert status="warning" className="macos-permissions-alert">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>{t("permissions.missingTitle")}</Alert.Title>
            <Alert.Description>{t("permissions.missingDescription")}</Alert.Description>
          </Alert.Content>
        </Alert>
      ) : (
        <Alert status="success" className="macos-permissions-alert">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>{t("permissions.readyTitle")}</Alert.Title>
            <Alert.Description>{t("permissions.readyDescription")}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {error ? (
        <Alert status="danger" className="macos-permissions-alert">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>{t("permissions.refreshErrorTitle")}</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      ) : null}

      {accessibilityMissing ? (
        <Alert status="default" className="macos-permissions-alert">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>{t("permissions.accessibilityHelpTitle")}</Alert.Title>
            <Alert.Description>{t("permissions.accessibilityHelpDescription")}</Alert.Description>
          </Alert.Content>
        </Alert>
      ) : null}

      <div className="macos-permissions-list">
        {permissionItems.map(({ id, Icon }) => {
          const item = permissions[id];
          const approved = isMacOSPermissionApproved(item.status);
          const chipColor = approved ? "success" : "warning";
          const chipLabel = approved
            ? t("permissions.status.approved")
            : t(`permissions.status.${item.status}`, {
                defaultValue: t("permissions.status.unknown"),
              });

          return (
            <div key={id} className="macos-permission-row">
              <span className="macos-permission-icon" aria-hidden="true">
                <Icon size={18} />
              </span>
              <div className="macos-permission-copy">
                <h4>{t(`permissions.items.${id}.title`)}</h4>
                <p>{t(`permissions.items.${id}.description`)}</p>
              </div>
              <Chip
                color={chipColor}
                variant="soft"
                size="sm"
                className="macos-permission-chip"
              >
                {approved ? <CheckCircle2 size={13} /> : <ShieldAlert size={13} />}
                <Chip.Label>{chipLabel}</Chip.Label>
              </Chip>
              <Button
                type="button"
                variant={approved ? "tertiary" : "secondary"}
                size="sm"
                onPress={() => onRequestPermission(id)}
                isDisabled={loading}
              >
                {approved ? t("permissions.checkAgain") : t("permissions.allow")}
              </Button>
            </div>
          );
        })}
      </div>

      <div className="macos-permissions-actions">
        <Button
          type="button"
          variant="secondary"
          onPress={onRefresh}
          isPending={loading}
        >
          <RefreshCw size={16} />
          {t("permissions.refresh")}
        </Button>
      </div>
    </div>
  );
}
