import type { ReactNode } from "react";
import {
  CheckCircle2,
  CircleAlert,
  Info,
  Keyboard,
  Mic,
  RefreshCw,
  ShieldAlert,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/motion/button/base";
import { cn } from "@/lib/utils";
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

type NoticeTone = "success" | "warning" | "danger" | "info";

const noticeStyles: Record<NoticeTone, { className: string; Icon: LucideIcon }> = {
  success: { className: "border-success/25 bg-success/10 text-success", Icon: CheckCircle2 },
  warning: { className: "border-warning/30 bg-warning/10 text-warning", Icon: ShieldAlert },
  danger: { className: "border-destructive/25 bg-destructive/10 text-destructive", Icon: CircleAlert },
  info: { className: "border-border bg-muted/50 text-muted-foreground", Icon: Info },
};

function Notice({ tone, title, children }: { tone: NoticeTone; title: string; children: ReactNode }) {
  const { className, Icon } = noticeStyles[tone];
  return (
    <div role="status" className={cn("flex gap-3 rounded-2xl border px-4 py-3", className)}>
      <Icon size={18} className="mt-0.5 shrink-0" aria-hidden="true" />
      <div className="min-w-0">
        <p className="font-medium text-foreground text-sm">{title}</p>
        <p className="mt-0.5 text-muted-foreground text-xs leading-5">{children}</p>
      </div>
    </div>
  );
}

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
    <div className="grid gap-4">
      {hasMissingPermission ? (
        <Notice tone="warning" title={t("permissions.missingTitle")}>
          {t("permissions.missingDescription")}
        </Notice>
      ) : (
        <Notice tone="success" title={t("permissions.readyTitle")}>
          {t("permissions.readyDescription")}
        </Notice>
      )}

      {error ? (
        <Notice tone="danger" title={t("permissions.refreshErrorTitle")}>
          {error}
        </Notice>
      ) : null}

      {accessibilityMissing ? (
        <Notice tone="info" title={t("permissions.accessibilityHelpTitle")}>
          {t("permissions.accessibilityHelpDescription")}
        </Notice>
      ) : null}

      <div className="divide-y divide-border overflow-hidden rounded-2xl border border-border">
        {permissionItems.map(({ id, Icon }) => {
          const item = permissions[id];
          const approved = isMacOSPermissionApproved(item.status);
          const chipLabel = approved
            ? t("permissions.status.approved")
            : t(`permissions.status.${item.status}`, {
                defaultValue: t("permissions.status.unknown"),
              });

          return (
            <div key={id} className="flex flex-wrap items-center gap-3 p-4">
              <span
                className="grid size-9 shrink-0 place-items-center rounded-full bg-muted text-muted-foreground"
                aria-hidden="true"
              >
                <Icon size={18} />
              </span>
              <div className="min-w-0 flex-1">
                <h3 className="font-medium text-sm">{t(`permissions.items.${id}.title`)}</h3>
                <p className="mt-0.5 text-muted-foreground text-xs leading-5">
                  {t(`permissions.items.${id}.description`)}
                </p>
              </div>
              <span
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2.5 py-1 font-medium text-xs",
                  approved ? "bg-success/10 text-success" : "bg-warning/10 text-warning",
                )}
              >
                {approved ? <CheckCircle2 size={13} /> : <ShieldAlert size={13} />}
                {chipLabel}
              </span>
              <Button
                variant={approved ? "ghost" : "secondary"}
                size="sm"
                onClick={() => onRequestPermission(id)}
                disabled={loading}
              >
                {approved ? t("permissions.checkAgain") : t("permissions.allow")}
              </Button>
            </div>
          );
        })}
      </div>

      <div>
        <Button variant="secondary" onClick={onRefresh} disabled={loading}>
          <RefreshCw size={16} className={cn(loading && "animate-spin")} />
          {t("permissions.refresh")}
        </Button>
      </div>
    </div>
  );
}
