import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/motion/button/base";
import { TextAreaField } from "./FormFields";

interface PromptTemplateEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function PromptTemplateEditor({
  value,
  onChange,
}: PromptTemplateEditorProps) {
  const { t } = useTranslation();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const insertVariable = (varName: string) => {
    const ta = textareaRef.current;
    if (!ta) return;

    const start = ta.selectionStart;
    const end = ta.selectionEnd;
    const placeholder = `{${varName}}`;
    const next = value.slice(0, start) + placeholder + value.slice(end);

    onChange(next);

    // 在 React 更新 DOM 后恢复光标到插入内容之后
    requestAnimationFrame(() => {
      ta.focus();
      ta.selectionStart = ta.selectionEnd = start + placeholder.length;
    });
  };

  return (
    <div className="grid gap-2">
      <TextAreaField
        label=""
        textareaRef={textareaRef}
        value={value}
        onChange={onChange}
        rows={3}
        spellCheck={false}
      />
      <div className="flex items-center gap-2 text-muted-foreground text-xs">
        <span>{t("triggers.insertPlaceholder")}</span>
        <div className="flex flex-wrap gap-1.5">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            className="font-mono"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => insertVariable("value")}
            aria-label={t("triggers.insertPlaceholderAria", { placeholder: "{value}" })}
          >
            {"{value}"}
          </Button>
        </div>
      </div>
    </div>
  );
}
