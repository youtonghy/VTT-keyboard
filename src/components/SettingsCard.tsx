import type { ReactNode } from "react";
import { Info } from "lucide-react";
import { Tooltip } from "./Tooltip";

interface SettingsCardProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function SettingsCard({ title, description, children }: SettingsCardProps) {
  return (
    <section className="settings-card">
      <header className="settings-card-header !mb-4">
        <div className="flex items-center gap-2">
          <h3 className="settings-card-title">{title}</h3>
          {description && (
            <Tooltip content={<div className="text-[13px] leading-relaxed max-w-[240px]">{description}</div>} position="top">
              <span className="flex items-center justify-center cursor-help text-[var(--color-text-secondary)] hover:text-[var(--color-accent-strong)] transition-colors p-1 -m-1">
                <Info size={16} />
              </span>
            </Tooltip>
          )}
        </div>
      </header>
      <div className="settings-card-body">{children}</div>
    </section>
  );
}
