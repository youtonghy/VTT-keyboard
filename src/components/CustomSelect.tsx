import { useMemo } from "react";
import { SettingsSelect } from "@/components/premium/settings-pages/settings-shared";
import type { SelectOption as BeuiSelectOption } from "@/components/premium/select/select";

export interface SelectOption {
  value: string;
  label: string;
  description?: string;
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
  ariaLabel?: string;
}

const toBeuiOption = (option: SelectOption, group?: string): BeuiSelectOption => ({
  id: option.value,
  label: option.label,
  description: option.description,
  group,
});

export function CustomSelect({
  value,
  options,
  groups,
  onChange,
  disabled = false,
  ariaLabel,
}: CustomSelectProps) {
  const beuiOptions = useMemo(
    () =>
      groups
        ? groups.flatMap((group) => group.options.map((option) => toBeuiOption(option, group.label)))
        : (options ?? []).map((option) => toBeuiOption(option)),
    [groups, options],
  );

  return (
    <SettingsSelect
      options={beuiOptions}
      value={value}
      onValueChange={onChange}
      disabled={disabled}
      label={ariaLabel}
      placeholder=""
    />
  );
}
