import type { ComponentProps, ReactNode, Ref } from "react";
import {
  Button,
  Checkbox,
  Input,
  Label,
  ListBox,
  Select,
  TextArea,
  TextField,
} from "@heroui/react";
import { Check } from "lucide-react";

export interface HeroSelectOption {
  value: string;
  label: string;
}

export interface HeroSelectOptionGroup {
  label: string;
  options: HeroSelectOption[];
}

interface HeroFieldProps {
  label: string;
  children: ReactNode;
}

interface HeroInputFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
}

interface HeroTextAreaFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  rows?: number;
  spellCheck?: boolean;
  textareaRef?: Ref<HTMLTextAreaElement>;
}

interface HeroCheckboxFieldProps {
  label: string;
  isSelected: boolean;
  onChange: (value: boolean) => void;
  children?: ReactNode;
}

interface HeroSelectFieldProps {
  label: string;
  value: string;
  options?: HeroSelectOption[];
  groups?: HeroSelectOptionGroup[];
  onChange: (value: string) => void;
  disabled?: boolean;
}

interface HeroSelectControlProps extends Omit<HeroSelectFieldProps, "label"> {
  ariaLabel?: string;
  label?: string;
}

export const heroInputClassName = "hero-input";

export function HeroField({ label, children }: HeroFieldProps) {
  return (
    <div className="field">
      <span>{label}</span>
      {children}
    </div>
  );
}

export function HeroInputField({
  label,
  value,
  onChange,
  type = "text",
  placeholder,
  disabled,
}: HeroInputFieldProps) {
  return (
    <TextField className="field" isDisabled={disabled}>
      <Label>{label}</Label>
      <Input
        className={heroInputClassName}
        fullWidth
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
    </TextField>
  );
}

export function HeroTextAreaField({
  label,
  value,
  onChange,
  rows = 3,
  spellCheck,
  textareaRef,
}: HeroTextAreaFieldProps) {
  return (
    <TextField className="field">
      <Label>{label}</Label>
      <TextArea
        ref={textareaRef}
        className="hero-textarea"
        fullWidth
        value={value}
        rows={rows}
        spellCheck={spellCheck}
        onChange={(event) => onChange(event.target.value)}
      />
    </TextField>
  );
}

export function HeroCheckboxField({
  label,
  isSelected,
  onChange,
  children,
}: HeroCheckboxFieldProps) {
  return (
    <Checkbox
      className="hero-checkbox"
      isSelected={isSelected}
      onChange={onChange}
    >
      <Checkbox.Control>
        <Checkbox.Indicator />
      </Checkbox.Control>
      <Checkbox.Content>
        <span>{label}</span>
        {children}
      </Checkbox.Content>
    </Checkbox>
  );
}

export function HeroSelectField({
  label,
  value,
  options,
  groups,
  onChange,
  disabled,
}: HeroSelectFieldProps) {
  return (
    <div className="field">
      <HeroSelectControl
        value={value}
        options={options}
        groups={groups}
        onChange={onChange}
        disabled={disabled}
        ariaLabel={label}
        label={label}
      />
    </div>
  );
}

export function HeroSelectControl({
  value,
  options,
  groups,
  onChange,
  disabled,
  ariaLabel,
  label,
}: HeroSelectControlProps) {
  const flatOptions = groups ? groups.flatMap((group) => group.options) : (options ?? []);
  const selectedOption = flatOptions.find((option) => option.value === value);

  return (
    <Select
      aria-label={ariaLabel ?? "Select"}
      selectedKey={value}
      isDisabled={disabled}
      onSelectionChange={(key) => {
        if (key != null) {
          onChange(String(key));
        }
      }}
    >
      {label ? <Label>{label}</Label> : null}
      <Select.Trigger className="hero-select-trigger">
        <Select.Value>{selectedOption?.label}</Select.Value>
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover className="hero-select-popover">
        <ListBox>
          {groups
            ? groups.map((group) => (
                <ListBox.Section key={group.label} aria-label={group.label}>
                  <div className="hero-select-group-label">{group.label}</div>
                  {group.options.map((option) => (
                    <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                      {option.label}
                      <ListBox.ItemIndicator>
                        <Check size={14} />
                      </ListBox.ItemIndicator>
                    </ListBox.Item>
                  ))}
                </ListBox.Section>
              ))
            : flatOptions.map((option) => (
                <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                  {option.label}
                  <ListBox.ItemIndicator>
                    <Check size={14} />
                  </ListBox.ItemIndicator>
                </ListBox.Item>
              ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}

export function HeroActionButton(props: ComponentProps<typeof Button>) {
  return <Button {...props} />;
}
