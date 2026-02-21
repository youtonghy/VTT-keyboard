import { useState, useRef, useEffect, DragEvent } from "react";

interface PromptTemplateEditorProps {
  value: string;
  onChange: (value: string) => void;
}

const DRAG_PLACEHOLDER = "⧼VALUE⧽";

export function PromptTemplateEditor({ value, onChange }: PromptTemplateEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const [html, setHtml] = useState("");
  const isComposing = useRef(false);
  const isDragging = useRef(false);

  // Parse plain text to HTML
  const textToHtml = (text: string) => {
    if (!text) return "";
    let result = text;
    // Escape HTML first
    result = result.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    
    // Replace newline with <br>
    result = result.replace(/\n/g, "<br>");
    
    // Replace {value} with spans
    const regex = new RegExp(`\\{value\\}`, 'g');
    result = result.replace(regex, `&#8203;<span class="prompt-variable" contenteditable="false" data-var="value" draggable="true">{value}</span>&#8203;`);
    
    return result;
  };

  // Parse HTML to plain text
  const htmlToText = (htmlElement: HTMLDivElement) => {
    let text = "";
    const nodes = htmlElement.childNodes;
    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
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
    return text.replace(/\u200B/g, ''); // Remove zero-width spaces
  };

  useEffect(() => {
    if (editorRef.current && document.activeElement !== editorRef.current && !isComposing.current) {
      setHtml(textToHtml(value));
    }
  }, [value]);

  const handleInput = () => {
    if (editorRef.current && !isComposing.current && !isDragging.current) {
      let newText = htmlToText(editorRef.current);
      
      // If we just dropped the placeholder, handle the logic
      if (newText.includes(DRAG_PLACEHOLDER)) {
        // Remove old {value} since we are moving it
        newText = newText.replace(/\{value\}/g, '');
        // Replace placeholder with {value}
        newText = newText.replace(new RegExp(DRAG_PLACEHOLDER, 'g'), '{value}');
        
        // Force a re-render to update the DOM with new HTML to clean up the mess
        setHtml(textToHtml(newText));
      }

      onChange(newText);
    }
  };

  const handleDragStart = (e: DragEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.classList && target.classList.contains("prompt-variable")) {
      isDragging.current = true;
      e.dataTransfer.setData("text/plain", DRAG_PLACEHOLDER);
      e.dataTransfer.effectAllowed = "move";
      // Slightly dim the original
      setTimeout(() => {
        target.style.opacity = "0.4";
      }, 0);
    }
  };

  const handleDragEnd = (e: DragEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.style) {
      target.style.opacity = "1";
    }
    isDragging.current = false;
    // We must call handleInput manually here because dropping plain text into contenteditable
    // sometimes triggers input, sometimes doesn't perfectly sync before dragEnd
    setTimeout(handleInput, 0);
  };

  const insertVariable = (varName: string) => {
    const editor = editorRef.current;
    if (!editor) return;
    editor.focus();
    const selection = window.getSelection();
    if (selection && selection.rangeCount > 0) {
      const range = selection.getRangeAt(0);
      range.deleteContents();
      const placeholder = document.createTextNode(`{${varName}}`);
      range.insertNode(placeholder);
      range.setStartAfter(placeholder);
      range.collapse(true);
      selection.removeAllRanges();
      selection.addRange(range);
    } else {
      // Fallback: append to end
      const newText = value + `{${varName}}`;
      onChange(newText);
      return;
    }
    // Trigger input handling to sync state
    setTimeout(handleInput, 0);
  };

  return (
    <div className="prompt-template-container">
      <div
        ref={editorRef}
        className="prompt-template-editor"
        contentEditable
        dangerouslySetInnerHTML={{ __html: html }}
        onInput={handleInput}
        onBlur={handleInput}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
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
