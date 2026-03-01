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
      // 注意：不加 draggable="true"，使用自定义鼠标事件替代 HTML5 Drag API
      '&#8203;<span class="prompt-variable" contenteditable="false" data-var="value">{value}</span>&#8203;'
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

  // 自定义拖拽状态
  const dragSourceRef = useRef<HTMLElement | null>(null);
  const dragActiveRef = useRef(false);
  const dragStartPos = useRef<{ x: number; y: number } | null>(null);

  // 始终持有最新 value/onChange，避免闭包陈旧引用
  const valueRef = useRef(value);
  valueRef.current = value;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  /**
   * Bug 1 修复：改用 useLayoutEffect 替代 useEffect。
   * useLayoutEffect 在 DOM 变更后、浏览器 paint 之前同步执行，
   * 确保 innerHTML 在首次绘制前就已填入，避免"空白→有内容"闪烁，
   * 从而消除用户感知到的"点击时意外插入 {value}"假象。
   */
  useLayoutEffect(() => {
    const el = editorRef.current;
    if (el && document.activeElement !== el && !isComposing.current) {
      el.innerHTML = textToHtml(value);
    }
  }, [value]);

  /**
   * Bug 2 修复：挂载时注册 document 级别鼠标事件，实现自定义拖拽。
   * 替代不可靠的 HTML5 Drag API（在 contentEditable 内部会直接移动 DOM 节点，
   * 绕过 dataTransfer，导致我们的逻辑无法介入）。
   *
   * 机制：
   *   mousedown on .prompt-variable → 记录源 span 和起始坐标
   *   document mousemove → 超过 4px 后激活拖拽，添加 .dragging 样式
   *   document mouseup  → 用 caretRangeFromXY 计算落点，在纯文本中执行移动
   */
  useLayoutEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      const source = dragSourceRef.current;
      if (!source) return;
      const start = dragStartPos.current;
      if (!start) return;

      const dx = e.clientX - start.x;
      const dy = e.clientY - start.y;

      // 超过 4px 才激活，防止普通点击误触
      if (!dragActiveRef.current && Math.sqrt(dx * dx + dy * dy) > 4) {
        dragActiveRef.current = true;
        source.classList.add("dragging");
        document.body.style.cursor = "grabbing";
      }

      // 激活后阻止文字选中
      if (dragActiveRef.current) {
        e.preventDefault();
      }
    };

    const onMouseUp = (e: MouseEvent) => {
      const source = dragSourceRef.current;
      const isActive = dragActiveRef.current;

      // 清理拖拽状态
      if (source) source.classList.remove("dragging");
      document.body.style.cursor = "";
      dragSourceRef.current = null;
      dragActiveRef.current = false;
      dragStartPos.current = null;

      const editor = editorRef.current;
      if (!isActive || !source || !editor) return;

      // 计算放置位置（兼容 Chrome/Safari/Firefox）
      const range = caretRangeFromXY(e.clientX, e.clientY);
      if (!range) return;

      const dropOff = getTextOffset(editor, range.startContainer, range.startOffset);
      const curText = htmlToText(editor);
      const srcIdx = findValueIndex(curText, source, editor);

      // 从文本中移除原始 {value}
      let next = curText.slice(0, srcIdx) + curText.slice(srcIdx + 7);

      // 若被移除片段在落点之前，需调整偏移量
      let adj = dropOff;
      if (srcIdx < dropOff) adj -= 7;
      adj = Math.max(0, Math.min(adj, next.length));

      // 在新位置插入 {value}
      next = next.slice(0, adj) + "{value}" + next.slice(adj);

      onChangeRef.current(next);
      editor.innerHTML = textToHtml(next);
      editor.focus();
      setCursorAtTextOffset(editor, adj + 7);
    };

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
    };
  }, []); // 仅在 mount/unmount 时执行

  /* ── 输入同步 ────────────────────────────────────────────────── */

  const handleInput = () => {
    const el = editorRef.current;
    if (el && !isComposing.current && !dragActiveRef.current) {
      onChange(htmlToText(el));
    }
  };

  /* ── 鼠标按下：检测是否点在 .prompt-variable 上以启动拖拽 ─── */

  const handleMouseDown = (e: React.MouseEvent<HTMLDivElement>) => {
    const t = e.target as HTMLElement;
    if (t.classList?.contains("prompt-variable")) {
      dragSourceRef.current = t;
      dragStartPos.current = { x: e.clientX, y: e.clientY };
    }
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
        onBlur={handleInput}
        onMouseDown={handleMouseDown}
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
