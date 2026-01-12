import type { ReactNode } from "react";

interface SettingsCardProps {
  title: string;
  description?: string;
  children: ReactNode;
}

export function SettingsCard({ title, description, children }: SettingsCardProps) {
  return (
    <section className="settings-card">
      <header>
        <h3>{title}</h3>
        {description ? <p>{description}</p> : null}
      </header>
      <div className="settings-card-body">{children}</div>
    </section>
  );
}
