import type { ReactNode } from "react";
import { Card } from "@heroui/react";
import { Info } from "lucide-react";
import { Tooltip } from "./Tooltip";

interface SettingsCardProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function SettingsCard({ title, description, children }: SettingsCardProps) {
  return (
    <Card className="settings-card">
      <Card.Header className="settings-card-header !mb-4">
        <div className="flex items-center gap-2">
          <Card.Title className="settings-card-title">{title}</Card.Title>
          {description && (
            <Tooltip content={<div className="text-[13px] leading-relaxed max-w-[240px]">{description}</div>} position="top">
              <span className="flex items-center justify-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors p-1 -m-1">
                <Info size={16} />
              </span>
            </Tooltip>
          )}
        </div>
      </Card.Header>
      <Card.Content className="settings-card-body">{children}</Card.Content>
    </Card>
  );
}
