import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { GreetingForm } from "./components/GreetingForm";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { LogoRow } from "./components/LogoRow";
import "./App.css";

function App() {
  const { t } = useTranslation();
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke<string>("greet", { name }));
  }

  return (
    <main className="container">
      <h1>{t("app.title")}</h1>
      <LogoRow />
      <p>{t("app.subtitle")}</p>
      <LanguageSwitcher />
      <GreetingForm name={name} onNameChange={setName} onSubmit={greet} />
      {greetMsg ? (
        <p className="greet-result">
          {t("greeting.resultLabel")}: {greetMsg}
        </p>
      ) : null}
    </main>
  );
}

export default App;
