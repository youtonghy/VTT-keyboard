import { 
  Settings, 
  Keyboard, 
  Mic, 
  MessageSquare, 
  Type, 
  Zap, 
  Info,
  History,
  ChevronLeft,
  ChevronRight,
  LucideIcon
} from "lucide-react";
import { useTranslation } from "react-i18next";

export interface SidebarItem {
  id: string;
  label: string;
}

interface SidebarProps {
  items: SidebarItem[];
  activeId: string;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onSelect: (id: string) => void;
}

const iconMap: Record<string, LucideIcon> = {
  general: Settings,
  shortcut: Keyboard,
  recording: Mic,
  speech: MessageSquare,
  text: Type,
  triggers: Zap,
  history: History,
  about: Info,
};

export function Sidebar({
  items,
  activeId,
  collapsed,
  onToggleCollapsed,
  onSelect,
}: SidebarProps) {
  const { t } = useTranslation();
  const toggleLabel = collapsed ? t("nav.expandSidebar") : t("nav.collapseSidebar");

  return (
    <nav 
      className={`sidebar ${collapsed ? "collapsed" : ""}`} 
      aria-label={t("nav.sectionsAria")}
    >
      <div className="sidebar-header">
        <button 
          type="button" 
          className="sidebar-collapse-btn"
          onClick={onToggleCollapsed}
          title={toggleLabel}
          aria-label={toggleLabel}
          aria-expanded={!collapsed}
        >
          {collapsed ? <ChevronRight size={18} /> : <ChevronLeft size={18} />}
        </button>
      </div>
      
      <div className="sidebar-nav">
        {items.map((item) => {
          const Icon = iconMap[item.id] || Settings;
          const isActive = activeId === item.id;
          
          return (
            <button
              key={item.id}
              type="button"
              className={`sidebar-nav-button ${isActive ? "active" : ""}`}
              aria-current={isActive ? "page" : undefined}
              onClick={() => onSelect(item.id)}
              title={collapsed ? item.label : undefined}
            >
              <Icon size={18} className="sidebar-icon" />
              <span className="sidebar-label">{item.label}</span>
            </button>
          );
        })}
      </div>
    </nav>
  );
}
