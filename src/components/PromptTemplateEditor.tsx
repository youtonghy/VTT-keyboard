import { useRef, useLayoutEffect } from "react";

interface PromptTemplateEditorProps {
  value: string;
  onChange: (value: string) => void;
}

/* ── pure helpers ──────────────────────────────────────────────────── */

/** 纯文本（含 `{value}` 占位符）→ 编辑器 innerHTML */
function textToHtml(text: string): string {
  if (!text) return "";
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\n/g, "<br>")
    .replace(
      /\{value\}/g,
      // ZWS 使光标可紧贴标签前后放置
      '&#8203;<span class="prompt-variable" contenteditable="false" data-var="value">{value}</span>&#8203;'
    );
}

/** 编辑器 DOM → 纯文本（清除 ZWS） */
function htmlToText(editor: HTMLDivElement): string {
  let text = "";
  for (let i = 0; i < editor.childNodes.length; i++) {
    const node = editor.childNodes[i];
    if (node.nodeType === Node.TEXT_NODE) {
      text += node.nodeValue;
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as HTMLElement;
      if (el.tagName === "BR") {
        text += "\n";
      } else if (el.classList.contains("prompt-variable")) {
        text += `{${el.dataset.var}}`;
      } else {
        text += el.innerText;
      }
    }
  }
  return text.replace(/\u200B/g, "");
}

/* ── component ─────────────────────────────────────────────────────── */

export function PromptTemplateEditor({
  value,
  onChange,
}: PromptTemplateEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const isComposing = useRef(false);
  const valueRef = useRef(value);
  valueRef.current = value;
  // 失焦时保存光标范围，供按钮插入时恢复
  const savedRangeRef = useRef<Range | null>(null);

  /**
   * useLayoutEffect：在浏览器 paint 前同步写入 innerHTML。
   * 避免挂载瞬间"空白→有内容"闪烁；仅在编辑器未聚焦时同步，
   * 防止外部 value 回写干扰正在编辑中的内容。
   */
  useLayoutEffect(() => {
    const el = editorRef.current;
    if (el && document.activeElement !== el && !isComposing.current) {
      el.innerHTML = textToHtml(value);
    }
  }, [value]);

  /* ── 输入同步 ────────────────────────────────────────────────── */

  const handleInput = () => {
    const el = editorRef.current;
    if (el && !isComposing.current) {
      onChange(htmlToText(el));
    }
  };

  const handleBlur = () => {
    // 失焦前保存光标位置，以便点击插入按钮后能还原
    const sel = window.getSelection();
    if (sel && sel.rangeCount > 0) {
      savedRangeRef.current = sel.getRangeAt(0).cloneRange();
    }
    handleInput();
  };

  /* ── 按钮插入变量 ───────────────────────────────────────────── */

  const insertVariable = (varName: string) => {
    const editor = editorRef.current;
    if (!editor) return;

    editor.focus();
    const sel = window.getSelection();
    if (!sel) return;

    // 还原失焦前保存的光标位置
    if (savedRangeRef.current) {
      sel.removeAllRanges();
      sel.addRange(savedRangeRef.current);
      savedRangeRef.current = null;
    }

    if (sel.rangeCount > 0 && editor.contains(sel.getRangeAt(0).startContainer)) {
      const range = sel.getRangeAt(0);
      range.deleteContents();

      // 构造：ZWS + <span.prompt-variable> + ZWS
      const zBefore = document.createTextNode("\u200B");
      const span = document.createElement("span");
      span.className = "prompt-variable";
      span.contentEditable = "false";
      span.dataset.var = varName;
      span.textContent = `{${varName}}`;
      const zAfter = document.createTextNode("\u200B");

      const frag = document.createDocumentFragment();
      frag.appendChild(zBefore);
      frag.appendChild(span);
      frag.appendChild(zAfter);
      range.insertNode(frag);

      // 光标置于插入标签之后
      const nr = document.createRange();
      nr.setStartAfter(zAfter);
      nr.collapse(true);
      sel.removeAllRanges();
      sel.addRange(nr);

      onChange(htmlToText(editor));
    } else {
      // 兜底：追加到末尾
      const nt = valueRef.current + `{${varName}}`;
      onChange(nt);
      editor.innerHTML = textToHtml(nt);
    }
  };

  /* ── render ─────────────────────────────────────────────────── */

  return (
    <div className="prompt-template-container">
      <div
        ref={editorRef}
        className="prompt-template-editor"
        contentEditable
        onInput={handleInput}
        onBlur={handleBlur}
        onCompositionStart={() => (isComposing.current = true)}
        onCompositionEnd={() => {
          isComposing.current = false;
          handleInput();
        }}
        suppressContentEditableWarning
      />
      <div className="prompt-variables-bar">
        <span className="prompt-variables-hint">插入占位符:</span>
        <div className="prompt-variables-list">
          <button
            type="button"
            className="prompt-variable-btn"
            onMouseDown={(e) => {
              // 阻止按钮获取焦点，使编辑器的 blur 不会在 insertVariable 前触发
              e.preventDefault();
            }}
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
