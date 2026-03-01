import { useRef } from "react";

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
      <textarea
        ref={textareaRef}
        className="prompt-template-editor"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={3}
        spellCheck={false}
      />
      <div className="prompt-variables-bar">
        <span className="prompt-variables-hint">插入占位符:</span>
        <div className="prompt-variables-list">
          <button
            type="button"
            className="prompt-variable-btn"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => insertVariable("value")}
            title="点击插入 {value}"
          >
            {"{value}"}
          </button>
        </div>
      </div>
    </div>
  );
}
