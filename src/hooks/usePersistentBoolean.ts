import { Dispatch, SetStateAction, useEffect, useState } from "react";

const parseBoolean = (raw: string | null, fallback: boolean) => {
  if (raw === "true") {
    return true;
  }
  if (raw === "false") {
    return false;
  }
  return fallback;
};

export function usePersistentBoolean(
  key: string,
  defaultValue: boolean
): [boolean, Dispatch<SetStateAction<boolean>>] {
  const [value, setValue] = useState<boolean>(() => {
    if (typeof window === "undefined") {
      return defaultValue;
    }
    try {
      return parseBoolean(window.localStorage.getItem(key), defaultValue);
    } catch {
      return defaultValue;
    }
  });

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    try {
      window.localStorage.setItem(key, String(value));
    } catch {
      // Ignore localStorage write failures (e.g. privacy mode).
    }
  }, [key, value]);

  return [value, setValue];
}
