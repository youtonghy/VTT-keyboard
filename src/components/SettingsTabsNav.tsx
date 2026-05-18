import { Tabs } from "@heroui/react";

export interface SettingsTabsNavItem {
  id: string;
  label: string;
}

interface SettingsTabsNavProps {
  items: SettingsTabsNavItem[];
  activeKey: string;
  ariaLabel: string;
  onActiveKeyChange: (key: string) => void;
}

export function SettingsTabsNav({
  items,
  activeKey,
  ariaLabel,
  onActiveKeyChange,
}: SettingsTabsNavProps) {
  return (
    <Tabs
      className="settings-tabs"
      selectedKey={activeKey}
      onSelectionChange={(key) => onActiveKeyChange(String(key))}
      variant="secondary"
    >
      <Tabs.ListContainer className="settings-tabs-list-container">
        <Tabs.List aria-label={ariaLabel} className="settings-tabs-list">
          {items.map((item) => (
            <Tabs.Tab key={item.id} id={item.id} className="settings-tabs-tab">
              {item.label}
              <Tabs.Indicator />
            </Tabs.Tab>
          ))}
        </Tabs.List>
      </Tabs.ListContainer>
    </Tabs>
  );
}
