import { useRef } from "react";
import { Button } from "@heroui/react";
import { HeroTextAreaField } from "./HeroField";

interface PromptTemplateEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function PromptTemplateEditor({
  value,
  onChange,
}: PromptTemplateEditorProps) {
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
    <div className="prompt-template-container">
      <HeroTextAreaField
        label=""
        textareaRef={textareaRef}
        value={value}
        onChange={onChange}
        rows={3}
        spellCheck={false}
      />
      <div className="prompt-variables-bar">
        <span className="prompt-variables-hint">插入占位符:</span>
        <div className="prompt-variables-list">
          <Button
            type="button"
            className="prompt-variable-btn"
            variant="secondary"
            onMouseDown={(e) => e.preventDefault()}
            onPress={() => insertVariable("value")}
            aria-label="点击插入 {value}"
          >
            {"{value}"}
          </Button>
        </div>
      </div>
    </div>
  );
}
