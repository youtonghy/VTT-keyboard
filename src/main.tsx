import React from "react";
import ReactDOM from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import App from "./App";
import StatusWindow from "./StatusWindow";
import i18n from "./i18n";

const isStatusWindow = window.location.hash === "#status";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nextProvider i18n={i18n}>
      {isStatusWindow ? <StatusWindow /> : <App />}
    </I18nextProvider>
  </React.StrictMode>,
);
