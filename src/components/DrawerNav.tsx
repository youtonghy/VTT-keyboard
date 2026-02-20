interface DrawerNavItem {
  id: string;
  label: string;
}

interface DrawerNavProps {
  items: DrawerNavItem[];
  activeId: string;
  onSelect: (id: string) => void;
}

export function DrawerNav({ items, activeId, onSelect }: DrawerNavProps) {
  return (
    <nav className="drawer-nav" aria-label="Sections">
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          className={activeId === item.id ? "drawer-nav-button active" : "drawer-nav-button"}
          aria-current={activeId === item.id ? "page" : undefined}
          onClick={() => onSelect(item.id)}
        >
          {item.label}
        </button>
      ))}
    </nav>
  );
}
