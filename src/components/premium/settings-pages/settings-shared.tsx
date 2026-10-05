"use client";

import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { type FormEvent, useRef, useState } from "react";
import { Button } from "@/components/motion/button/base";
import { Select, type SelectProps } from "@/components/premium/select/select";
import { EASE_OUT } from "@/lib/ease";
import { cn } from "@/lib/utils";

export const settingsInput =
  "mt-2 h-10 w-full rounded-full border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50";

export function useSettingsForm<T>(
  initial: T,
  onSave?: (value: T) => void | Promise<void>,
) {
  const [value, setValue] = useState(initial);
  const [saved, setSaved] = useState(initial);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">(
    "idle",
  );
  const lock = useRef(false);
  const dirty = JSON.stringify(value) !== JSON.stringify(saved);
  function update(next: T) {
    setValue(next);
    setStatus("idle");
  }
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (lock.current || !dirty) return;
    lock.current = true;
    setStatus("saving");
    try {
      await onSave?.(value);
      setSaved(value);
      setStatus("saved");
    } catch {
      setStatus("error");
    } finally {
      lock.current = false;
    }
  }
  return {
    value,
    update,
    status,
    dirty,
    submit,
    reset: () => {
      setValue(saved);
      setStatus("idle");
    },
  };
}

export function SettingsFooter({
  dirty,
  status,
  reset,
  demo,
}: {
  dirty: boolean;
  status: "idle" | "saving" | "saved" | "error";
  reset: () => void;
  demo: boolean;
}) {
  const reduce = useReducedMotion();
  const message =
    status === "error"
      ? "Couldn’t save your changes. Please try again."
      : status === "saving"
        ? "Saving changes…"
        : status === "saved"
          ? demo
            ? "Changes saved for this preview."
            : "Your changes are saved."
          : dirty
            ? "You have unsaved changes."
            : "You’re all up to date.";
  return (
    <footer className="flex flex-wrap items-center justify-between gap-4 border-t border-border pt-6">
      <output aria-live="polite" className="text-sm text-muted-foreground">
        <AnimatePresence mode="wait" initial={false}>
          <motion.span
            key={message}
            initial={{ opacity: 0, y: reduce ? 0 : 4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: reduce ? 0 : -4 }}
            transition={{ duration: reduce ? 0 : 0.15, ease: EASE_OUT }}
          >
            {message}
          </motion.span>
        </AnimatePresence>
      </output>
      <div className="flex gap-2">
        <Button
          variant="ghost"
          onClick={reset}
          disabled={!dirty || status === "saving"}
          className="focus-visible:ring-2 focus-visible:ring-ring rounded-full h-9"
        >
          Discard
        </Button>
        <Button
          type="submit"
          disabled={!dirty || status === "saving"}
          className="focus-visible:ring-2 focus-visible:ring-ring rounded-full h-9"
        >
          {status === "saving" ? "Saving…" : "Save changes"}
        </Button>
      </div>
    </footer>
  );
}

export function SettingsSelect({
  className,
  buttonClassName,
  optionsClassName,
  ...props
}: SelectProps) {
  return (
    <Select
      {...props}
      portal
      className={cn("block w-full", className)}
      buttonClassName={cn(
        "h-10 w-full min-w-0 justify-between rounded-full bg-background text-sm font-normal focus-visible:ring-2 focus-visible:ring-ring",
        buttonClassName,
      )}
      optionsClassName={cn(
        "w-[var(--button-width)] min-w-40 max-w-[calc(100vw-1.5rem)] shadow-none motion-reduce:transition-none motion-reduce:data-closed:scale-100",
        optionsClassName,
      )}
    />
  );
}
