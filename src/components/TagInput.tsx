import { useEffect, useState } from "react";

interface TagInputProps {
  values: string[];
  onCommit: (values: string[]) => void;
  placeholder?: string;
  disabled?: boolean;
}

const listToString = (values: string[]) => values.join(", ");

const parseList = (value: string) =>
  value
    .split(/[,，]/)
    .map((item) => item.trim())
    .filter(Boolean);

const areListsEqual = (left: string[], right: string[]) =>
  left.length === right.length && left.every((value, index) => value === right[index]);

export function TagInput({ values, onCommit, placeholder, disabled }: TagInputProps) {
  const [inputValue, setInputValue] = useState(() => listToString(values));
  const [isEditing, setIsEditing] = useState(false);

  useEffect(() => {
    if (!isEditing) {
      setInputValue(listToString(values));
    }
  }, [isEditing, values]);

  const handleBlur = () => {
    setIsEditing(false);
    if (disabled) {
      return;
    }
    const nextValues = parseList(inputValue);
    const normalized = listToString(nextValues);
    setInputValue(normalized);
    if (!areListsEqual(nextValues, values)) {
      onCommit(nextValues);
    }
  };

  return (
    <input
      type="text"
      value={inputValue}
      placeholder={placeholder}
      disabled={disabled}
      onChange={(event) => setInputValue(event.target.value)}
      onFocus={() => setIsEditing(true)}
      onBlur={handleBlur}
    />
  );
}
