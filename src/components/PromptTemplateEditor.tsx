import { useRef, useEffect, DragEvent } from "react";

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
      '&#8203;<span class="prompt-variable" contenteditable="false" data-var="value" draggable="true">{value}</span>&#8203;'
    );
}

/** 编辑器 DOM → 纯文本 */
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

/**
 * 将 DOM 位置（node + offset）映射为纯文本字符偏移量（忽略零宽空格）。
 */
function getTextOffset(
  editor: HTMLDivElement,
  targetNode: Node,
  targetOffset: number
): number {
  let n = 0;
  let found = false;

  function count(node: Node): void {
    if (node.nodeType === Node.TEXT_NODE) {
      n += (node.nodeValue || "").replace(/\u200B/g, "").length;
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as HTMLElement;
      if (el.classList?.contains("prompt-variable")) {
        n += `{${el.dataset.var}}`.length;
      } else if (el.tagName === "BR") {
        n += 1;
      } else {
        for (let i = 0; i < node.childNodes.length; i++) count(node.childNodes[i]);
      }
    }
  }

  function walk(node: Node): void {
    if (found) return;
    if (node === targetNode) {
      if (node.nodeType === Node.TEXT_NODE) {
        n += (node.nodeValue || "")
          .slice(0, targetOffset)
          .replace(/\u200B/g, "").length;
      } else {
        for (let i = 0; i < targetOffset && i < node.childNodes.length; i++) {
          count(node.childNodes[i]);
        }
      }
      found = true;
      return;
    }
    if (node.nodeType === Node.TEXT_NODE) {
      n += (node.nodeValue || "").replace(/\u200B/g, "").length;
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as HTMLElement;
      if (el.classList?.contains("prompt-variable")) {
        n += `{${el.dataset.var}}`.length;
      } else if (el.tagName === "BR") {
        n += 1;
      } else {
        for (let i = 0; i < node.childNodes.length; i++) {
          walk(node.childNodes[i]);
          if (found) return;
        }
      }
    }
  }

  for (let i = 0; i < editor.childNodes.length; i++) {
    walk(editor.childNodes[i]);
    if (found) break;
  }
  return n;
}

/** 将光标移动到编辑器内指定的纯文本偏移量处。 */
function setCursorAtTextOffset(editor: HTMLDivElement, target: number): void {
  const sel = window.getSelection();
  if (!sel) return;

  let offset = 0;
  for (let i = 0; i < editor.childNodes.length; i++) {
    const node = editor.childNodes[i];

    if (node.nodeType === Node.TEXT_NODE) {
      const raw = node.nodeValue || "";
      const clean = raw.replace(/\u200B/g, "");
      if (offset + clean.length >= target) {
        const needed = target - offset;
        let rawIdx = 0;
        let cleanCount = 0;
        for (let j = 0; j < raw.length; j++) {
          if (raw[j] !== "\u200B") {
            if (cleanCount >= needed) break;
            cleanCount++;
          }
          rawIdx = j + 1;
        }
        const r = document.createRange();
        r.setStart(node, rawIdx);
        r.collapse(true);
        sel.removeAllRanges();
        sel.addRange(r);
        return;
      }
      offset += clean.length;
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as HTMLElement;
      if (el.classList?.contains("prompt-variable")) {
        const len = `{${el.dataset.var}}`.length;
        if (offset + len >= target) {
          const r = document.createRange();
          r.setStartAfter(el);
          r.collapse(true);
          sel.removeAllRanges();
          sel.addRange(r);
          return;
        }
        offset += len;
      } else if (el.tagName === "BR") {
        if (offset >= target) {
          const r = document.createRange();
          r.setStart(editor, i);
          r.collapse(true);
          sel.removeAllRanges();
          sel.addRange(r);
          return;
        }
        offset += 1;
      }
    }
  }

  // 兜底：光标放到末尾
  const r = document.createRange();
  r.selectNodeContents(editor);
  r.collapse(false);
  sel.removeAllRanges();
  sel.addRange(r);
}

/**
 * 返回指定 span（可能存在多个 {value}）在纯文本中的起始索引。
 */
function findValueIndex(
  text: string,
  sourceSpan: HTMLElement,
  editor: HTMLDivElement
): number {
  const spans = editor.querySelectorAll(".prompt-variable");
  let spanIdx = 0;
  for (let i = 0; i < spans.length; i++) {
    if (spans[i] === sourceSpan) {
      spanIdx = i;
      break;
    }
  }
  let idx = 0;
  for (let i = 0; i <= spanIdx; i++) {
    const pos = text.indexOf("{value}", idx);
    if (pos === -1) return 0;
    if (i === spanIdx) return pos;
    idx = pos + 7;
  }
  return 0;
}

/** 兼容 Chromium/Safari 与 Firefox 的光标位置计算。 */
function caretRangeFromXY(x: number, y: number): Range | null {
  if (document.caretRangeFromPoint) {
    return document.caretRangeFromPoint(x, y);
  }
  const pos = (document as any).caretPositionFromPoint?.(x, y);
  if (pos) {
    const range = document.createRange();
    range.setStart(pos.offsetNode, pos.offset);
    range.collapse(true);
    return range;
  }
  return null;
}

/* ── component ─────────────────────────────────────────────────────── */

export function PromptTemplateEditor({
  value,
  onChange,
}: PromptTemplateEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const isComposing = useRef(false);
  const dragSourceRef = useRef<HTMLElement | null>(null);
  // 始终持有最新 value，避免闭包陈旧引用
  const valueRef = useRef(value);
  valueRef.current = value;

  /**
   * 通过 ref 直接写 innerHTML，绕开 React reconciliation，
   * 避免 contentEditable 光标被重置。
   */
  const syncHtml = (text: string) => {
    if (editorRef.current) editorRef.current.innerHTML = textToHtml(text);
  };

  // 挂载时初始化；外部 value 变化（且编辑器未聚焦）时同步
  useEffect(() => {
    const el = editorRef.current;
    if (el && document.activeElement !== el && !isComposing.current) {
      syncHtml(value);
    }
  }, [value]);

  /* ── 输入同步 ────────────────────────────────────────────────── */

  const handleInput = () => {
    const el = editorRef.current;
    if (el && !isComposing.current && !dragSourceRef.current) {
      onChange(htmlToText(el));
    }
  };

  /* ── 拖拽 ───────────────────────────────────────────────────── */

  const onDragStart = (e: DragEvent<HTMLDivElement>) => {
    const t = e.target as HTMLElement;
    if (t.classList?.contains("prompt-variable")) {
      dragSourceRef.current = t;
      e.dataTransfer.setData("text/plain", "{value}");
      e.dataTransfer.effectAllowed = "move";
      t.classList.add("dragging");
    }
  };

  const onDragOver = (e: DragEvent<HTMLDivElement>) => {
    if (dragSourceRef.current) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
    }
  };

  const onDrop = (e: DragEvent<HTMLDivElement>) => {
    const editor = editorRef.current;
    const source = dragSourceRef.current;
    if (!editor || !source) return;

    // 阻止浏览器默认的 DOM 节点移动行为
    e.preventDefault();
    e.stopPropagation();

    const range = caretRangeFromXY(e.clientX, e.clientY);
    if (!range) {
      source.classList.remove("dragging");
      dragSourceRef.current = null;
      return;
    }

    const dropOff = getTextOffset(editor, range.startContainer, range.startOffset);
    const curText = htmlToText(editor);
    const srcIdx = findValueIndex(curText, source, editor);

    // 从文本中移除原始 {value}
    let next = curText.slice(0, srcIdx) + curText.slice(srcIdx + 7);

    // 如果被移除的片段在放置点之前，调整偏移量
    let adj = dropOff;
    if (srcIdx < dropOff) adj -= 7;
    adj = Math.max(0, Math.min(adj, next.length));

    // 在新位置插入 {value}
    next = next.slice(0, adj) + "{value}" + next.slice(adj);

    dragSourceRef.current = null;
    onChange(next);
    syncHtml(next);

    // 将光标定位到插入标签之后
    editor.focus();
    setCursorAtTextOffset(editor, adj + 7);
  };

  const onDragEnd = (e: DragEvent<HTMLDivElement>) => {
    (e.target as HTMLElement).classList?.remove("dragging");
    dragSourceRef.current = null;
  };

  /* ── 按钮插入变量 ───────────────────────────────────────────── */

  const insertVariable = (varName: string) => {
    const editor = editorRef.current;
    if (!editor) return;

    editor.focus();
    const sel = window.getSelection();

    if (sel && sel.rangeCount > 0) {
      const range = sel.getRangeAt(0);
      range.deleteContents();

      // 构造：ZWS + <span.prompt-variable> + ZWS
      const zBefore = document.createTextNode("\u200B");
      const span = document.createElement("span");
      span.className = "prompt-variable";
      span.contentEditable = "false";
      span.dataset.var = varName;
      span.draggable = true;
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
      syncHtml(nt);
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
        onBlur={handleInput}
        onDragStart={onDragStart}
        onDragOver={onDragOver}
        onDrop={onDrop}
        onDragEnd={onDragEnd}
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
