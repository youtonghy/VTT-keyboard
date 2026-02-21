
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
          <button
            key={option.value}
            type="button"
            className={`segmented-control-button ${isActive ? "active" : ""}`}
            onClick={() => !disabled && onChange(option.value)}
            disabled={disabled}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
