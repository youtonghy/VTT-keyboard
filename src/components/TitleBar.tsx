import { getCurrentWindow } from "@tauri-apps/api/window";
import { useState, useEffect } from "react";
import "../App.css"; // Ensure styles are available

export function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const updateState = async () => {
      setIsMaximized(await getCurrentWindow().isMaximized());
    };
    
    updateState();
    const unlisten = getCurrentWindow().listen("tauri://resize", updateState);
    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const minimize = () => getCurrentWindow().minimize();
  const toggleMaximize = async () => {
    await getCurrentWindow().toggleMaximize();
    setIsMaximized(!isMaximized);
  };
  const close = () => getCurrentWindow().close();

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-title">VTT Keyboard</div>
      <div className="titlebar-controls">
        <button className="titlebar-button" onClick={minimize} title="Minimize">
          <svg width="10" height="1" viewBox="0 0 10 1">
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button className="titlebar-button" onClick={toggleMaximize} title="Maximize">
          {isMaximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M2,0 L10,0 L10,8 L8,8 L8,10 L0,10 L0,2 L2,2 L2,0 Z M8,2 L8,8 L2,8 L2,2 L8,2 Z" fill="currentColor" fillRule="evenodd"/>
            </svg>
          ) : (
             <svg width="10" height="10" viewBox="0 0 10 10">
              <rect width="10" height="10" stroke="currentColor" strokeWidth="1" fill="none" />
            </svg>
          )}
        </button>
        <button className="titlebar-button close" onClick={close} title="Close">
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path d="M0,0 L10,10 M10,0 L0,10" stroke="currentColor" strokeWidth="1.2" />
          </svg>
        </button>
      </div>
    </div>
  );
}
