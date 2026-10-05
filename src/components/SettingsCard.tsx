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
    <section className="rounded-3xl border border-border bg-card p-5 sm:p-6">
      <header className="mb-5 flex items-center gap-2">
        <h2 className="font-semibold text-base text-card-foreground tracking-tight">{title}</h2>
        {description ? (
          <Tooltip
            content={<div className="max-w-[240px] text-[13px] leading-relaxed">{description}</div>}
            position="top"
          >
            <span className="-m-1 flex cursor-help items-center p-1 text-muted-foreground transition-colors hover:text-foreground">
              <Info size={15} />
            </span>
          </Tooltip>
        ) : null}
      </header>
      <div className="grid gap-5">{children}</div>
    </section>
  );
}
