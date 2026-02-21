import { useState } from "react";
import { 
  Settings, 
  Keyboard, 
  Mic, 
  MessageSquare, 
  Type, 
  Zap, 
  Info,
  ChevronLeft,
  ChevronRight,
  LucideIcon
} from "lucide-react";

export interface SidebarItem {
  id: string;
  label: string;
}

interface SidebarProps {
  items: SidebarItem[];
  activeId: string;
  onSelect: (id: string) => void;
}

const iconMap: Record<string, LucideIcon> = {
  general: Settings,
  shortcut: Keyboard,
  recording: Mic,
  speech: MessageSquare,
  text: Type,
  triggers: Zap,
  about: Info,
};

export function Sidebar({ items, activeId, onSelect }: SidebarProps) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <nav 
      className={`sidebar ${collapsed ? "collapsed" : ""}`} 
      aria-label="Sections"
    >
      <div className="sidebar-header">
        <button 
          type="button" 
          className="sidebar-collapse-btn"
          onClick={() => setCollapsed(!collapsed)}
          title={collapsed ? "展开侧边栏" : "收起侧边栏"}
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
