import { useState, useRef, useEffect } from "react";

interface PromptTemplateEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function PromptTemplateEditor({ value, onChange }: PromptTemplateEditorProps) {
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
    if (editorRef.current && !isComposing.current) {
      const newText = htmlToText(editorRef.current);
      onChange(newText);
    }
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
        onDrop={() => setTimeout(handleInput, 0)}
        onCompositionStart={() => (isComposing.current = true)}
        onCompositionEnd={() => {
          isComposing.current = false;
          handleInput();
        }}
        suppressContentEditableWarning
      />
    </div>
  );
}