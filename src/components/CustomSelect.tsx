import { HeroSelectControl } from "./HeroField";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectOptionGroup {
  label: string;
  options: SelectOption[];
}

interface CustomSelectProps {
  value: string;
  options?: SelectOption[];
  groups?: SelectOptionGroup[];
  onChange: (value: string) => void;
  disabled?: boolean;
}

export function CustomSelect({ value, options, groups, onChange, disabled = false }: CustomSelectProps) {
  return (
    <HeroSelectControl
      value={value}
      options={options}
      groups={groups}
      onChange={onChange}
      disabled={disabled}
    />
  );
}
