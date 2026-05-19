
import { Button } from "@heroui/react";

export interface SegmentedControlOption {
  value: string;
  label: string;
}

interface SegmentedControlProps {
  value: string;
  options: SegmentedControlOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
}

export function SegmentedControl({ value, options, onChange, disabled }: SegmentedControlProps) {
  return (
    <div className={`segmented-control ${disabled ? "disabled" : ""}`}>
      {options.map((option) => {
        const isActive = option.value === value;
        return (
          <Button
            key={option.value}
            type="button"
            className={`segmented-control-button ${isActive ? "active" : ""}`}
            variant={isActive ? "primary" : "ghost"}
            onPress={() => !disabled && onChange(option.value)}
            isDisabled={disabled}
          >
            {option.label}
          </Button>
        );
      })}
    </div>
  );
}
