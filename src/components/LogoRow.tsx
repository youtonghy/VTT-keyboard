import reactLogo from "../assets/react.svg";

export function LogoRow() {
  return (
    <div className="row logo-row">
      <a href="https://vite.dev" target="_blank" rel="noreferrer">
        <img src="/vite.svg" className="logo vite" alt="Vite logo" />
      </a>
      <a href="https://tauri.app" target="_blank" rel="noreferrer">
        <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
      </a>
      <a href="https://react.dev" target="_blank" rel="noreferrer">
        <img src={reactLogo} className="logo react" alt="React logo" />
      </a>
    </div>
  );
}
