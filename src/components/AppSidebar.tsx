import {
  History,
  Info,
  Keyboard,
  MessageSquare,
  Mic,
  Settings,
  ShieldCheck,
  Type,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  AnimatedSidebar,
  AnimatedSidebarClose,
  AnimatedSidebarContent,
  AnimatedSidebarFooter,
  AnimatedSidebarGroup,
  AnimatedSidebarGroupContent,
  AnimatedSidebarHeader,
  AnimatedSidebarMenu,
  AnimatedSidebarMenuButton,
  AnimatedSidebarMenuItem,
} from "@/components/motion/animated-sidebar";

export interface AppSidebarItem {
  id: string;
  label: string;
}

interface AppSidebarProps {
  items: AppSidebarItem[];
  activeId: string;
  onSelect: (id: string) => void;
  version?: string;
}

const iconMap: Record<string, LucideIcon> = {
  general: Settings,
  shortcut: Keyboard,
  permissions: ShieldCheck,
  recording: Mic,
  speech: MessageSquare,
  text: Type,
  triggers: Zap,
  history: History,
  about: Info,
};

export function AppSidebar({ items, activeId, onSelect, version }: AppSidebarProps) {
  const { t } = useTranslation();

  return (
    <AnimatedSidebar
      ariaLabel={t("nav.sectionsAria")}
      collapsible="icon"
      className="min-h-0"
      panelClassName="h-full bg-sidebar border-sidebar-border"
    >
      <AnimatedSidebarHeader className="p-3 pb-2">
        <div className="flex min-h-11 items-center gap-3 overflow-hidden px-2">
          <span className="grid size-7 shrink-0 place-items-center rounded-full bg-foreground text-background">
            <Mic aria-hidden="true" className="size-4" />
          </span>
          <span className="min-w-0 flex-1 truncate font-semibold text-foreground text-sm group-data-[state=collapsed]/sidebar:hidden">
            VTT Keyboard
          </span>
          <AnimatedSidebarClose className="ml-auto text-muted-foreground hover:bg-muted md:hidden">
            <X aria-hidden="true" className="size-4" />
          </AnimatedSidebarClose>
        </div>
      </AnimatedSidebarHeader>

      <AnimatedSidebarContent className="px-2 pt-1">
        <AnimatedSidebarGroup>
          <AnimatedSidebarGroupContent>
            <AnimatedSidebarMenu>
              {items.map((item) => {
                const Icon = iconMap[item.id] ?? Settings;
                return (
                  <AnimatedSidebarMenuItem key={item.id}>
                    <AnimatedSidebarMenuButton
                      isActive={activeId === item.id}
                      icon={<Icon className="size-4" />}
                      onSelect={() => onSelect(item.id)}
                    >
                      {item.label}
                    </AnimatedSidebarMenuButton>
                  </AnimatedSidebarMenuItem>
                );
              })}
            </AnimatedSidebarMenu>
          </AnimatedSidebarGroupContent>
        </AnimatedSidebarGroup>
      </AnimatedSidebarContent>

      {version ? (
        <AnimatedSidebarFooter className="border-none p-3">
          <p className="truncate px-2 text-muted-foreground text-xs group-data-[state=collapsed]/sidebar:hidden">
            v{version}
          </p>
        </AnimatedSidebarFooter>
      ) : null}
    </AnimatedSidebar>
  );
}
