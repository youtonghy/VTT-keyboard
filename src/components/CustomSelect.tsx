import { useState, useRef, useEffect, useMemo } from "react";
import { ChevronDown, Check } from "lucide-react";

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
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const flatOptions = useMemo(() => {
    if (groups) {
      return groups.flatMap((g) => g.options);
    }
    return options ?? [];
  }, [options, groups]);

  const selectedOption = flatOptions.find((opt) => opt.value === value) || flatOptions[0];

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  useEffect(() => {
    if (disabled) {
      setIsOpen(false);
    }
  }, [disabled]);

  useEffect(() => {
    setIsOpen(false);
  }, [value]);

  const handleSelect = (nextValue: string) => {
    setIsOpen(false);
    if (nextValue !== value) {
      onChange(nextValue);
    }
  };

  const renderOption = (option: SelectOption) => {
    const isSelected = option.value === value;
    return (
      <li key={option.value}>
        <button
          type="button"
          className={`custom-select-option ${isSelected ? "selected" : ""}`}
          role="option"
          aria-selected={isSelected}
          onClick={() => handleSelect(option.value)}
        >
          <span className="custom-select-option-label">{option.label}</span>
          {isSelected && <Check size={16} className="custom-select-check" />}
        </button>
      </li>
    );
  };

  return (
    <div className={`custom-select-container ${disabled ? "disabled" : ""}`} ref={containerRef}>
      <button
        type="button"
        className={`custom-select-trigger ${isOpen ? "open" : ""}`}
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
      >
        <span className="custom-select-value">{selectedOption?.label}</span>
        <ChevronDown size={16} className={`custom-select-icon ${isOpen ? "rotate" : ""}`} />
      </button>

      {isOpen && !disabled && (
        <div className="custom-select-dropdown">
          <ul className="custom-select-options" role="listbox">
            {groups
              ? groups.map((group) => (
                  <li key={group.label} role="presentation">
                    <span className="custom-select-group-label">{group.label}</span>
                    <ul role="group" aria-label={group.label}>
                      {group.options.map(renderOption)}
                    </ul>
                  </li>
                ))
              : flatOptions.map(renderOption)}
          </ul>
        </div>
      )}
    </div>
  );
}
