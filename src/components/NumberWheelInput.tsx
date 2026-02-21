import { useState, useEffect, useRef } from "react";

interface NumberWheelInputProps {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
}

export function NumberWheelInput({
  value,
  onChange,
  min,
  max,
  step = 1,
  disabled = false,
}: NumberWheelInputProps) {
  const [internalValue, setInternalValue] = useState(value.toString());
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (document.activeElement !== inputRef.current) {
      setInternalValue(value.toString());
    }
  }, [value]);

  const commitValue = (valStr: string) => {
    let num = parseFloat(valStr);
    if (isNaN(num)) {
      setInternalValue(value.toString());
      return;
    }
    if (min !== undefined && num < min) num = min;
    if (max !== undefined && num > max) num = max;
    
    onChange(num);
  };

  useEffect(() => {
    const el = inputRef.current;
    if (!el) return;

    const handleWheel = (e: WheelEvent) => {
      if (disabled) return;
      e.preventDefault();
      
      const direction = e.deltaY < 0 ? 1 : -1;
      let num = parseFloat(el.value) || 0;
      num += direction * step;
      
      if (min !== undefined && num < min) num = min;
      if (max !== undefined && num > max) num = max;
      
      const decimals = step.toString().split(".")[1]?.length || 0;
      num = parseFloat(num.toFixed(decimals));
      
      setInternalValue(num.toString());
      onChange(num);
    };

    el.addEventListener("wheel", handleWheel, { passive: false });
    return () => el.removeEventListener("wheel", handleWheel);
  }, [step, min, max, disabled, onChange]);

  return (
    <input
      ref={inputRef}
      type="number"
      value={internalValue}
      min={min}
      max={max}
      step={step}
      disabled={disabled}
      onChange={(e) => setInternalValue(e.target.value)}
      onBlur={(e) => commitValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          commitValue(e.currentTarget.value);
        }
      }}
    />
  );
}
