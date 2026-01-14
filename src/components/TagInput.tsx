import { useState, type KeyboardEvent } from "react";

interface TagInputProps {
  values: string[];
  onChange: (values: string[]) => void;
  placeholder?: string;
  disabled?: boolean;
}

const splitTags = (value: string) =>
  value
    .split(/[,，]/)
    .map((item) => item.trim())
    .filter(Boolean);

const appendUnique = (current: string[], incoming: string[]) => {
  const existing = new Set(current.map((value) => value.toLowerCase()));
  const next = [...current];
  for (const item of incoming) {
    const normalized = item.toLowerCase();
    if (!existing.has(normalized)) {
      existing.add(normalized);
      next.push(item);
    }
  }
  return next;
};

export function TagInput({ values, onChange, placeholder, disabled }: TagInputProps) {
  const [inputValue, setInputValue] = useState("");

  const commitValue = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed) {
      setInputValue("");
      return;
    }
    const tags = splitTags(trimmed);
    if (tags.length === 0) {
      setInputValue("");
      return;
    }
    onChange(appendUnique(values, tags));
    setInputValue("");
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" || event.key === "," || event.key === "，") {
      event.preventDefault();
      commitValue(inputValue);
      return;
    }
    if (event.key === "Backspace" && inputValue.trim() === "" && values.length > 0) {
      event.preventDefault();
      onChange(values.slice(0, -1));
    }
  };

  const handleBlur = () => {
    commitValue(inputValue);
  };

  const handleRemove = (value: string) => {
    onChange(values.filter((item) => item !== value));
  };

  return (
    <div className={disabled ? "tag-input disabled" : "tag-input"}>
      {values.map((value) => (
        <span key={value} className="tag-input-item">
          {value}
          <button
            type="button"
            className="tag-input-remove"
            onClick={() => handleRemove(value)}
            disabled={disabled}
            aria-label={value}
          >
            ×
          </button>
        </span>
      ))}
      <input
        type="text"
        value={inputValue}
        placeholder={placeholder}
        onChange={(event) => setInputValue(event.target.value)}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        disabled={disabled}
      />
    </div>
  );
}
