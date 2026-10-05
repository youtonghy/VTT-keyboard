"use client";

import {
  Listbox,
  ListboxButton,
  ListboxOption,
  ListboxOptions,
} from "@headlessui/react";
import { Check, ChevronDown, type LucideIcon } from "lucide-react";
import { Fragment, useMemo, useState } from "react";
import { cn } from "@/lib/utils";

export type SelectOption = {
  id: string;
  label: string;
  description?: string;
  icon?: LucideIcon;
  disabled?: boolean;
  /** Options sharing a group are rendered under one heading, in order. */
  group?: string;
};

export type SelectProps = {
  options: readonly SelectOption[];
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  placeholder?: string;
  label?: string;
  align?: "start" | "end";
  side?: "top" | "bottom";
  portal?: boolean;
  disabled?: boolean;
  className?: string;
  buttonClassName?: string;
  optionsClassName?: string;
  optionClassName?: string;
};

export function Select({
  options,
  value,
  defaultValue,
  onValueChange,
  placeholder = "Select option",
  label,
  align = "start",
  side = "bottom",
  portal = false,
  disabled,
  className,
  buttonClassName,
  optionsClassName,
  optionClassName,
}: SelectProps) {
  const [internalValue, setInternalValue] = useState(
    defaultValue ?? options[0]?.id ?? "",
  );
  const controlled = value !== undefined;
  const selectedValue = controlled ? value : internalValue;
  const selected = useMemo(
    () => options.find((option) => option.id === selectedValue),
    [options, selectedValue],
  );

  function setValue(nextValue: string) {
    if (!controlled) setInternalValue(nextValue);
    onValueChange?.(nextValue);
  }

  return (
    <Listbox
      as="div"
      value={selectedValue}
      onChange={setValue}
      disabled={disabled}
      className={cn("relative inline-block text-left", className)}
    >
      {({ open }) => (
        <>
          <ListboxButton
            aria-label={label}
            className={cn(
              "inline-flex h-10 min-w-40 items-center justify-between gap-2 rounded-full border border-border bg-background px-3 text-left font-medium text-foreground text-sm transition-colors",
              "hover:border-border-strong disabled:pointer-events-none disabled:opacity-50",
              "outline-none ring-0 focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0 data-focus:outline-none data-focus:ring-0 data-open:ring-0",
              buttonClassName,
            )}
          >
            <span className="min-w-0 truncate">
              {selected?.label ?? placeholder}
            </span>
            <ChevronDown
              className={cn(
                "size-4 shrink-0 text-muted-foreground transition-transform",
                open && "rotate-180",
              )}
            />
          </ListboxButton>

          <ListboxOptions
            transition
            portal={portal}
            anchor={
              portal
                ? {
                    to: `${side} ${align}` as
                      | "top start"
                      | "top end"
                      | "bottom start"
                      | "bottom end",
                    gap: 8,
                    padding: 12,
                  }
                : undefined
            }
            className={cn(
              "z-50 overflow-hidden rounded-2xl border border-border-strong/50 bg-background p-1.5 backdrop-blur-xl",
              "outline-none ring-0 focus:outline-none focus:ring-0 focus-visible:outline-none focus-visible:ring-0 data-focus:outline-none data-focus:ring-0",
              "transition duration-150 ease-out data-closed:scale-95 data-closed:opacity-0",
              side === "top" ? "origin-bottom" : "origin-top",
              !portal && [
                "absolute",
                side === "top" ? "bottom-full mb-2" : "top-full mt-2",
                align === "end" ? "right-0" : "left-0",
              ],
              "w-[var(--button-width)]",
              optionsClassName,
              "max-w-[calc(100vw-1.5rem)]",
            )}
          >
            {options.map((option, index) => (
              <Fragment key={option.id}>
                {option.group && option.group !== options[index - 1]?.group ? (
                  <div className="px-3 pt-2 pb-1 font-medium text-muted-foreground text-xs">
                    {option.group}
                  </div>
                ) : null}
                <SelectMenuOption
                  option={option}
                  optionClassName={optionClassName}
                />
              </Fragment>
            ))}
          </ListboxOptions>
        </>
      )}
    </Listbox>
  );
}

function SelectMenuOption({
  option,
  optionClassName,
}: {
  option: SelectOption;
  optionClassName?: string;
}) {
  const Icon = option.icon;

  return (
    <ListboxOption
      value={option.id}
      disabled={option.disabled}
      className={cn(
        "group flex min-h-11 cursor-pointer items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors",
        "data-focus:bg-primary/5 data-disabled:pointer-events-none data-disabled:opacity-50",
        optionClassName,
      )}
    >
      {Icon ? (
        <span className="grid size-8 shrink-0 place-items-center text-muted-foreground">
          <Icon className="size-4" />
        </span>
      ) : null}
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm">{option.label}</span>
        {option.description ? (
          <span className="block truncate text-muted-foreground text-xs">
            {option.description}
          </span>
        ) : null}
      </span>
      <Check className="size-4 shrink-0 opacity-0 group-data-selected:opacity-100" />
    </ListboxOption>
  );
}
