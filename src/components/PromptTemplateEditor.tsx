import React, { useState, useRef, useEffect } from "react";


interface PromptTemplateEditorProps {
  value: string;
  variables: string[];
  onChange: (value: string) => void;
}

export function PromptTemplateEditor({ value, variables, onChange }: PromptTemplateEditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const [html, setHtml] = useState("");
  const isComposing = useRef(false);

  // Parse plain text to HTML
  const textToHtml = (text: string) => {
    if (!text) return "";
    let result = text;
    // Escape HTML first
    result = result.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    
    // Replace newline with <br>
    result = result.replace(/\n/g, "<br>");
    
    // Replace {variable} with spans
    variables.forEach(v => {
      const regex = new RegExp(`\\{${v}\\}`, 'g');
      result = result.replace(regex, `<span class="prompt-variable" contenteditable="false" data-var="${v}" draggable="true">{${v}}</span>`);
    });
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
    return text;
  };

  useEffect(() => {
    if (editorRef.current && document.activeElement !== editorRef.current && !isComposing.current) {
      setHtml(textToHtml(value));
    }
  }, [value, variables]);

  const handleInput = () => {
    if (editorRef.current && !isComposing.current) {
      const newText = htmlToText(editorRef.current);
      onChange(newText);
    }
  };

  // Handle drag and drop for variables
  const handleDragStart = (e: React.DragEvent) => {
    const target = e.target as HTMLElement;
    if (target.classList.contains('prompt-variable')) {
      e.dataTransfer.setData('text/plain', `{${target.dataset.var}}`);
      e.dataTransfer.effectAllowed = 'copyMove';
      setTimeout(() => {
        target.classList.add('dragging');
      }, 0);
    }
  };

  const handleDragEnd = (e: React.DragEvent) => {
    const target = e.target as HTMLElement;
    if (target.classList.contains('prompt-variable')) {
      target.classList.remove('dragging');
    }
    handleInput(); // Re-sync after drop
  };

  const insertVariable = (variable: string) => {
    if (!editorRef.current) return;
    editorRef.current.focus();
    
    const selection = window.getSelection();
    if (!selection || selection.rangeCount === 0) return;
    
    let range = selection.getRangeAt(0);
    
    // Ensure we're inserting inside the editor
    if (!editorRef.current.contains(range.commonAncestorContainer)) {
      range = document.createRange();
      range.selectNodeContents(editorRef.current);
      range.collapse(false);
      selection.removeAllRanges();
      selection.addRange(range);
    }

    const span = document.createElement('span');
    span.className = "prompt-variable";
    span.contentEditable = "false";
    span.dataset.var = variable;
    span.draggable = true;
    span.textContent = `{${variable}}`;

    range.deleteContents();
    range.insertNode(span);
    
    // Move cursor after the inserted node
    range.setStartAfter(span);
    range.collapse(true);
    selection.removeAllRanges();
    selection.addRange(range);
    
    handleInput();
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
        onCompositionStart={() => (isComposing.current = true)}
        onCompositionEnd={() => {
          isComposing.current = false;
          handleInput();
        }}
        onDragStart={handleDragStart}
        onDragEnd={handleDragEnd}
        suppressContentEditableWarning
      />
      {variables.length > 0 && (
        <div className="prompt-variables-bar">
          <span className="prompt-variables-hint">插入变量:</span>
          <div className="prompt-variables-list">
            {variables.map(v => (
              <button
                key={v}
                type="button"
                className="prompt-variable-btn"
                onClick={() => insertVariable(v)}
                title={`点击插入 {${v}}`}
              >
                {v}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
