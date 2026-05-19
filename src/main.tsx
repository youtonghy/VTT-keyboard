import React from "react";
import ReactDOM from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import { setupDevTauriMocks } from "./devTauriMocks";
import "./styles/tailwind.css";

async function renderApp() {
  await setupDevTauriMocks();

  const [{ default: App }, { default: i18n }] = await Promise.all([
    import("./App"),
    import("./i18n"),
  ]);

  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <I18nextProvider i18n={i18n}>
        <App />
      </I18nextProvider>
    </React.StrictMode>,
  );
}

void renderApp();
