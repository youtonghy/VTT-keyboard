import { useId, type ReactNode, type Ref } from "react";
import { Switch } from "@/components/premium/switch/switch";
import { cn } from "@/lib/utils";

export const inputClassName =
  "h-10 w-full min-w-0 rounded-full border border-border bg-background px-4 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground hover:border-border-strong focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50";

export const textAreaClassName =
  "w-full min-w-0 resize-y rounded-2xl border border-border bg-background px-4 py-3 text-sm leading-6 text-foreground outline-none transition-colors placeholder:text-muted-foreground hover:border-border-strong focus-visible:ring-2 focus-visible:ring-ring";

interface FieldProps {
  label: string;
  htmlFor?: string;
  children: ReactNode;
  className?: string;
}

/** 标签在上、控件在下的通用表单行。 */
export function Field({ label, htmlFor, children, className }: FieldProps) {
  return (
    <div className={cn("grid gap-2", className)}>
      {label ? (
        <label htmlFor={htmlFor} className="font-medium text-foreground text-sm">
          {label}
        </label>
      ) : null}
      {children}
    </div>
  );
}

interface InputFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
}

export function InputField({
  label,
  value,
  onChange,
  type = "text",
  placeholder,
  disabled,
}: InputFieldProps) {
  const id = useId();
  return (
    <Field label={label} htmlFor={id}>
      <input
        id={id}
        className={inputClassName}
        type={type}
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    </Field>
  );
}

interface TextAreaFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  rows?: number;
  spellCheck?: boolean;
  textareaRef?: Ref<HTMLTextAreaElement>;
}

export function TextAreaField({
  label,
  value,
  onChange,
  rows = 3,
  spellCheck,
  textareaRef,
}: TextAreaFieldProps) {
  const id = useId();
  return (
    <Field label={label} htmlFor={id}>
      <textarea
        id={id}
        ref={textareaRef}
        className={textAreaClassName}
        value={value}
        rows={rows}
        spellCheck={spellCheck}
        onChange={(event) => onChange(event.target.value)}
      />
    </Field>
  );
}

interface ToggleFieldProps {
  label: string;
  isSelected: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
  /** 跟在标签后的附加内容，通常是说明 Tooltip。 */
  children?: ReactNode;
}

/** 左侧标签、右侧开关的设置行。 */
export function ToggleField({
  label,
  isSelected,
  onChange,
  disabled,
  children,
}: ToggleFieldProps) {
  return (
    <div className="flex min-h-10 items-center justify-between gap-4">
      <div className="flex min-w-0 items-center gap-1.5 text-foreground text-sm">
        <span className="min-w-0">{label}</span>
        {children}
      </div>
      <Switch
        checked={isSelected}
        onCheckedChange={onChange}
        disabled={disabled}
        size="sm"
        aria-label={label}
      />
    </div>
  );
}
