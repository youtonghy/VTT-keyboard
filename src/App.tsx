import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { DrawerNav } from "./components/DrawerNav";
import { LanguageSwitcher } from "./components/LanguageSwitcher";
import { SettingsCard } from "./components/SettingsCard";
import { TitleBar } from "./components/TitleBar";
import { useSettings } from "./hooks/useSettings";
import type { Settings } from "./types/settings";
import "./App.css";

const listToString = (values: string[]) => values.join(", ");
const parseList = (value: string) =>
  value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);

const modifierKeys = new Set(["Shift", "Control", "Alt", "Meta"]);

const logDebug = (...args: unknown[]) => {
  if (import.meta.env.DEV) {
    console.log("[shortcut]", ...args);
  }
};

const logError = (...args: unknown[]) => {
  if (import.meta.env.DEV) {
    console.error("[shortcut]", ...args);
  }
};

const toErrorMessage = (error: unknown) => {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
};

const isConflictError = (message: string) => {
  const lowered = message.toLowerCase();
  return (
    lowered.includes("already") ||
    lowered.includes("registered") ||
    lowered.includes("conflict") ||
    lowered.includes("in use")
  );
};

const normalizeShortcutKey = (key: string) => {
  if (key === " ") {
    return "Space";
  }
  if (key.startsWith("Arrow")) {
    return key.replace("Arrow", "");
  }
  if (key.length === 1) {
    return key.toUpperCase();
  }
  return key;
};

const buildShortcut = (event: KeyboardEvent) => {
  const parts: string[] = [];
  if (event.metaKey || event.ctrlKey) {
    parts.push("CommandOrControl");
  }
  if (event.altKey) {
    parts.push("Alt");
  }
  if (event.shiftKey) {
    parts.push("Shift");
  }
  const key = normalizeShortcutKey(event.key);
  parts.push(key);
  return parts.join("+");
};

const createId = () =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;

function App() {
  const { t, i18n } = useTranslation();
  const { settings, loading, saveSettings } = useSettings();
  const [draft, setDraft] = useState<Settings | null>(null);
  const [activeSection, setActiveSection] = useState("general");
  const [message, setMessage] = useState<{
    type: "success" | "error";
    text: string;
  } | null>(null);
  const [isCapturing, setIsCapturing] = useState(false);

  useEffect(() => {
    if (settings) {
      setDraft(settings);
    }
  }, [settings]);

  useEffect(() => {
    if (!draft) {
      return;
    }
    const root = document.documentElement;
    if (draft.appearance.theme === "system") {
      const media = window.matchMedia("(prefers-color-scheme: dark)");
      const applyTheme = () => {
        root.setAttribute("data-theme", media.matches ? "dark" : "light");
      };
      applyTheme();
      media.addEventListener("change", applyTheme);
      return () => media.removeEventListener("change", applyTheme);
    }
    root.setAttribute("data-theme", draft.appearance.theme);
  }, [draft?.appearance.theme]);

  useEffect(() => {
    if (!import.meta.env.DEV) {
      return;
    }
    const stopListening = listen<string>("dev-log", (event) => {
      console.log("[vtt]", event.payload);
    });
    stopListening.catch((error) => {
      console.error("[vtt] listen failed", error);
    });
    return () => {
      void stopListening.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (!message) {
      return;
    }
    const timer = window.setTimeout(() => setMessage(null), 2000);
    return () => window.clearTimeout(timer);
  }, [message]);

  useEffect(() => {
    if (!draft) {
      return;
    }
    let active = true;

    const registerShortcut = async () => {
      try {
        await unregisterAll();
        logDebug("unregister all success");
      } catch (error) {
        logError("unregister all failed", error);
        setMessage({
          type: "error",
          text: t("shortcut.unregisterError"),
        });
      }

      try {
        await register(draft.shortcut.key, (event: { state: string }) => {
          if (!active) {
            return;
          }
          logDebug("event", event.state);
          if (event.state === "Pressed") {
            invoke("start_recording")
              .then(() => logDebug("start_recording ok"))
              .catch((error) => {
                const message = toErrorMessage(error);
                logError("start_recording failed", message);
                setMessage({
                  type: "error",
                  text: t("shortcut.startError", { error: message }),
                });
              });
          }
          if (event.state === "Released") {
            invoke("stop_recording")
              .then(() => logDebug("stop_recording ok"))
              .catch((error) => {
                const message = toErrorMessage(error);
                logError("stop_recording failed", message);
                setMessage({
                  type: "error",
                  text: t("shortcut.stopError", { error: message }),
                });
              });
          }
        });
        logDebug("register success", draft.shortcut.key);
      } catch (error) {
        const message = toErrorMessage(error);
        logError("register failed", message);
        if (isConflictError(message)) {
          setMessage({
            type: "error",
            text: t("shortcut.conflict", { shortcut: draft.shortcut.key }),
          });
        } else {
          setMessage({
            type: "error",
            text: t("shortcut.registerError", { error: message }),
          });
        }
      }
    };

    void registerShortcut();

    return () => {
      active = false;
      unregisterAll()
        .then(() => logDebug("unregister all cleanup"))
        .catch((error) => logError("unregister cleanup failed", error));
    };
  }, [draft?.shortcut.key, t]);

  useEffect(() => {
    if (!isCapturing) {
      return;
    }
    const handleKeydown = (event: KeyboardEvent) => {
      if (modifierKeys.has(event.key)) {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const hasModifier = event.ctrlKey || event.metaKey || event.altKey || event.shiftKey;
      if (!hasModifier) {
        setMessage({ type: "error", text: t("shortcut.requireModifier") });
        setIsCapturing(false);
        return;
      }
      const shortcut = buildShortcut(event);
      updateDraft((prev) => ({
        ...prev,
        shortcut: { ...prev.shortcut, key: shortcut },
      }));
      setIsCapturing(false);
      setMessage({
        type: "success",
        text: t("shortcut.captureSuccess", { shortcut }),
      });
    };
    window.addEventListener("keydown", handleKeydown, true);
    return () => window.removeEventListener("keydown", handleKeydown, true);
  }, [isCapturing, t]);

  useEffect(() => {
    void invoke("set_tray_menu", {
      labels: {
        showSettings: t("tray.showSettings"),
        quit: t("tray.quit"),
      },
    });
  }, [i18n.language, t]);

  const navItems = useMemo(
    () => [
      { id: "general", label: t("nav.general") },
      { id: "shortcut", label: t("nav.shortcut") },
      { id: "recording", label: t("nav.recording") },
      { id: "speech", label: t("nav.speech") },
      { id: "text", label: t("nav.text") },
      { id: "triggers", label: t("nav.triggers") },
    ],
    [t]
  );

  const updateDraft = (updater: (prev: Settings) => Settings) => {
    setDraft((prev) => (prev ? updater(prev) : prev));
  };

  const validateTriggers = (next: Settings) => {
    const invalidRange = next.triggers.find((card) =>
      card.variables.every((value) => value.trim().length === 0)
    );
    if (invalidRange) {
      return t("triggers.validationVariables", { title: invalidRange.title });
    }
    const invalidKeyword = next.triggers.find((card) => card.keyword.trim().length === 0);
    if (invalidKeyword) {
      return t("triggers.validationKeyword", { title: invalidKeyword.title });
    }
    return null;
  };

  const createTriggerCard = () => ({
    id: createId(),
    title: t("triggers.newTitle"),
    enabled: true,
    autoApply: false,
    locked: false,
    keyword: t("triggers.defaultKeyword"),
    promptTemplate: t("triggers.defaultTemplate"),
    variables: parseList(t("triggers.defaultVariables")),
    example: t("triggers.defaultExample"),
    description: t("triggers.defaultDescription"),
  });

  const updateTrigger = (
    id: string,
    updater: (card: Settings["triggers"][number]) => Settings["triggers"][number]
  ) => {
    updateDraft((prev) => ({
      ...prev,
      triggers: prev.triggers.map((card) => (card.id === id ? updater(card) : card)),
    }));
  };

  const moveTrigger = (from: number, to: number) => {
    updateDraft((prev) => {
      const next = [...prev.triggers];
      const [item] = next.splice(from, 1);
      next.splice(to, 0, item);
      return { ...prev, triggers: next };
    });
  };

  const removeTrigger = (id: string) => {
    updateDraft((prev) => ({
      ...prev,
      triggers: prev.triggers.filter((card) => card.id !== id || card.locked),
    }));
  };

  const addTrigger = () => {
    updateDraft((prev) => ({
      ...prev,
      triggers: [...prev.triggers, createTriggerCard()],
    }));
  };

  const handleSave = async () => {
    if (!draft) {
      return;
    }
    const error = validateTriggers(draft);
    if (error) {
      setMessage({ type: "error", text: error });
      return;
    }
    try {
      await saveSettings(draft);
      setMessage({ type: "success", text: t("actions.saveSuccess") });
    } catch (err) {
      setMessage({ type: "error", text: t("actions.saveError") });
    }
  };

  const handleImport = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const selected = Array.isArray(path) ? path[0] : path;
    if (!selected) {
      return;
    }
    try {
      const data = await invoke<Settings>("import_settings", { path: selected });
      setDraft(data);
      setMessage({ type: "success", text: t("data.importSuccess") });
    } catch (err) {
      setMessage({ type: "error", text: t("data.importError") });
    }
  };

  const handleExport = async () => {
    const path = await save({
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) {
      return;
    }
    try {
      await invoke("export_settings", { path });
      setMessage({ type: "success", text: t("data.exportSuccess") });
    } catch (err) {
      setMessage({ type: "error", text: t("data.exportError") });
    }
  };

  if (loading || !draft) {
    return (
      <>
        <TitleBar />
        <main className="container loading">
          <p>{t("app.loading")}</p>
        </main>
      </>
    );
  }

  return (
    <>
      <TitleBar />
      <main className="container">
        <div className="settings-layout">
          <DrawerNav
            items={navItems}
            activeId={activeSection}
            onSelect={setActiveSection}
          />
          <section className="settings-content">
            {activeSection === "general" ? (
              <>
                <SettingsCard
                  title={t("general.title")}
                  description={t("general.description")}
                >
                  <label className="field">
                    <span>{t("general.theme")}</span>
                    <select
                      value={draft.appearance.theme}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          appearance: {
                            ...prev.appearance,
                            theme: event.target.value,
                          },
                        }))
                      }
                    >
                      <option value="system">{t("general.themeSystem")}</option>
                      <option value="light">{t("general.themeLight")}</option>
                      <option value="dark">{t("general.themeDark")}</option>
                    </select>
                  </label>
                  <div className="field">
                    <span>{t("general.language")}</span>
                    <LanguageSwitcher />
                  </div>
                </SettingsCard>
                <SettingsCard title={t("data.title")} description={t("data.description")}>
                  <div className="button-row">
                    <button type="button" onClick={handleImport}>
                      {t("data.import")}
                    </button>
                    <button type="button" onClick={handleExport}>
                      {t("data.export")}
                    </button>
                  </div>
                </SettingsCard>
              </>
            ) : null}

            {activeSection === "shortcut" ? (
              <SettingsCard
                title={t("shortcut.title")}
                description={t("shortcut.description")}
              >
                <label className="field">
                  <span>{t("shortcut.key")}</span>
                  <input
                    value={draft.shortcut.key}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        shortcut: { ...prev.shortcut, key: event.target.value },
                      }))
                    }
                  />
                </label>
                <div className="shortcut-actions">
                  <button
                    type="button"
                    onClick={() => setIsCapturing(true)}
                    disabled={isCapturing}
                  >
                    {isCapturing ? t("shortcut.capturing") : t("shortcut.capture")}
                  </button>
                  <span>{t("shortcut.captureHint")}</span>
                </div>
              </SettingsCard>
            ) : null}

            {activeSection === "recording" ? (
              <SettingsCard
                title={t("recording.title")}
                description={t("recording.description")}
              >
                <label className="field">
                  <span>{t("recording.segmentSeconds")}</span>
                  <input
                    type="number"
                    min={10}
                    value={draft.recording.segmentSeconds}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        recording: {
                          ...prev.recording,
                          segmentSeconds: Number(event.target.value),
                        },
                      }))
                    }
                  />
                </label>
              </SettingsCard>
            ) : null}

            {activeSection === "speech" ? (
              <>
                <SettingsCard
                  title={t("speech.title")}
                  description={t("speech.description")}
                >
                  <label className="field">
                    <span>{t("speech.provider")}</span>
                    <select
                      value={draft.provider}
                      onChange={(event) =>
                        updateDraft((prev) => ({
                          ...prev,
                          provider: event.target.value as "openai" | "volcengine",
                        }))
                      }
                    >
                      <option value="openai">OpenAI</option>
                      <option value="volcengine">{t("speech.volcengine")}</option>
                    </select>
                  </label>
                </SettingsCard>

                {draft.provider === "openai" ? (
                  <SettingsCard title="OpenAI">
                    <label className="field">
                      <span>{t("openai.apiBase")}</span>
                      <input
                        value={draft.openai.apiBase}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: { ...prev.openai, apiBase: event.target.value },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("openai.apiKey")}</span>
                      <input
                        type="password"
                        value={draft.openai.apiKey}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: { ...prev.openai, apiKey: event.target.value },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.model")}</span>
                      <input
                        value={draft.openai.speechToText.model}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                model: event.target.value,
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.language")}</span>
                      <input
                        value={draft.openai.speechToText.language}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                language: event.target.value,
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.prompt")}</span>
                      <input
                        value={draft.openai.speechToText.prompt}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                prompt: event.target.value,
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.responseFormat")}</span>
                      <input
                        value={draft.openai.speechToText.responseFormat}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                responseFormat: event.target.value,
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.temperature")}</span>
                      <input
                        type="number"
                        step="0.1"
                        value={draft.openai.speechToText.temperature}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                temperature: Number(event.target.value),
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.chunkingStrategy")}</span>
                      <input
                        value={draft.openai.speechToText.chunkingStrategy}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                chunkingStrategy: event.target.value,
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.timestampGranularities")}</span>
                      <input
                        value={listToString(draft.openai.speechToText.timestampGranularities)}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                timestampGranularities: parseList(event.target.value),
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.include")}</span>
                      <input
                        value={listToString(draft.openai.speechToText.include)}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                include: parseList(event.target.value),
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.knownSpeakerNames")}</span>
                      <input
                        value={listToString(draft.openai.speechToText.knownSpeakerNames)}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                knownSpeakerNames: parseList(event.target.value),
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("speech.knownSpeakerReferences")}</span>
                      <input
                        value={listToString(draft.openai.speechToText.knownSpeakerReferences)}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                knownSpeakerReferences: parseList(event.target.value),
                              },
                            },
                          }))
                        }
                      />
                    </label>
                    <label className="field checkbox">
                      <input
                        type="checkbox"
                        checked={draft.openai.speechToText.stream}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            openai: {
                              ...prev.openai,
                              speechToText: {
                                ...prev.openai.speechToText,
                                stream: event.target.checked,
                              },
                            },
                          }))
                        }
                      />
                      <span>{t("speech.stream")}</span>
                    </label>
                  </SettingsCard>
                ) : null}

                {draft.provider === "volcengine" ? (
                  <SettingsCard title={t("speech.volcengine")}>
                    <label className="field">
                      <span>{t("volcengine.appId")}</span>
                      <input
                        value={draft.volcengine.appId}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            volcengine: { ...prev.volcengine, appId: event.target.value },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("volcengine.accessToken")}</span>
                      <input
                        type="password"
                        value={draft.volcengine.accessToken}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            volcengine: { ...prev.volcengine, accessToken: event.target.value },
                          }))
                        }
                      />
                    </label>
                    <label className="field">
                      <span>{t("volcengine.language")}</span>
                      <select
                        value={draft.volcengine.language}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            volcengine: { ...prev.volcengine, language: event.target.value },
                          }))
                        }
                      >
                        <option value="zh-CN">{t("volcengine.langZhCN")}</option>
                        <option value="zh-TW">{t("volcengine.langZhTW")}</option>
                        <option value="en-US">{t("volcengine.langEnUS")}</option>
                        <option value="ja-JP">{t("volcengine.langJaJP")}</option>
                        <option value="ko-KR">{t("volcengine.langKoKR")}</option>
                      </select>
                    </label>
                    <label className="field checkbox">
                      <input
                        type="checkbox"
                        checked={draft.volcengine.useStreaming}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            volcengine: { ...prev.volcengine, useStreaming: event.target.checked },
                          }))
                        }
                      />
                      <span>{t("volcengine.useStreaming")}</span>
                    </label>
                    <label className="field checkbox">
                      <input
                        type="checkbox"
                        checked={draft.volcengine.useFast}
                        onChange={(event) =>
                          updateDraft((prev) => ({
                            ...prev,
                            volcengine: { ...prev.volcengine, useFast: event.target.checked },
                          }))
                        }
                      />
                      <span>{t("volcengine.useFast")}</span>
                    </label>
                  </SettingsCard>
                ) : null}
              </>
            ) : null}

            {activeSection === "text" ? (
              <SettingsCard title={t("text.title")} description={t("text.description")}>
                <label className="field">
                  <span>{t("text.model")}</span>
                  <input
                    value={draft.openai.text.model}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: { ...prev.openai.text, model: event.target.value },
                        },
                      }))
                    }
                  />
                </label>
                <label className="field">
                  <span>{t("text.instructions")}</span>
                  <input
                    value={draft.openai.text.instructions}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: {
                            ...prev.openai.text,
                            instructions: event.target.value,
                          },
                        },
                      }))
                    }
                  />
                </label>
                <label className="field">
                  <span>{t("text.temperature")}</span>
                  <input
                    type="number"
                    step="0.1"
                    value={draft.openai.text.temperature}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: {
                            ...prev.openai.text,
                            temperature: Number(event.target.value),
                          },
                        },
                      }))
                    }
                  />
                </label>
                <label className="field">
                  <span>{t("text.maxOutputTokens")}</span>
                  <input
                    type="number"
                    min={1}
                    value={draft.openai.text.maxOutputTokens}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: {
                            ...prev.openai.text,
                            maxOutputTokens: Number(event.target.value),
                          },
                        },
                      }))
                    }
                  />
                </label>
                <label className="field">
                  <span>{t("text.topP")}</span>
                  <input
                    type="number"
                    step="0.1"
                    value={draft.openai.text.topP}
                    onChange={(event) =>
                      updateDraft((prev) => ({
                        ...prev,
                        openai: {
                          ...prev.openai,
                          text: {
                            ...prev.openai.text,
                            topP: Number(event.target.value),
                          },
                        },
                      }))
                    }
                  />
                </label>
              </SettingsCard>
            ) : null}

            {activeSection === "triggers" ? (
              <SettingsCard
                title={t("triggers.title")}
                description={t("triggers.description")}
              >
                <div className="trigger-list">
                  {draft.triggers.map((card, index) => (
                    <div key={card.id} className="trigger-card">
                      <div className="trigger-card-header">
                        <input
                          value={card.title}
                          onChange={(event) =>
                            updateTrigger(card.id, (prev) => ({
                              ...prev,
                              title: event.target.value,
                            }))
                          }
                        />
                        <div className="trigger-card-actions">
                          <button
                            type="button"
                            onClick={() => moveTrigger(index, index - 1)}
                            disabled={index === 0}
                          >
                            {t("triggers.moveUp")}
                          </button>
                          <button
                            type="button"
                            onClick={() => moveTrigger(index, index + 1)}
                            disabled={index === draft.triggers.length - 1}
                          >
                            {t("triggers.moveDown")}
                          </button>
                          <button
                            type="button"
                            onClick={() => removeTrigger(card.id)}
                            disabled={card.locked}
                          >
                            {t("triggers.remove")}
                          </button>
                        </div>
                      </div>
                      <div className="trigger-card-body">
                        <label className="field checkbox">
                          <input
                            type="checkbox"
                            checked={card.enabled}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                enabled: event.target.checked,
                              }))
                            }
                          />
                          <span>{t("triggers.enabled")}</span>
                        </label>
                        <label className="field checkbox">
                          <input
                            type="checkbox"
                            checked={card.autoApply}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                autoApply: event.target.checked,
                              }))
                            }
                          />
                          <span>{t("triggers.autoApply")}</span>
                        </label>
                        <label className="field">
                          <span>{t("triggers.keyword")}</span>
                          <input
                            value={card.keyword}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                keyword: event.target.value,
                              }))
                            }
                          />
                        </label>
                        <label className="field">
                          <span>{t("triggers.variables")}</span>
                          <input
                            value={listToString(card.variables)}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                variables: parseList(event.target.value),
                              }))
                            }
                          />
                        </label>
                        <label className="field">
                          <span>{t("triggers.promptTemplate")}</span>
                          <input
                            value={card.promptTemplate}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                promptTemplate: event.target.value,
                              }))
                            }
                          />
                        </label>
                        <label className="field">
                          <span>{t("triggers.example")}</span>
                          <input
                            value={card.example}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                example: event.target.value,
                              }))
                            }
                          />
                        </label>
                        <label className="field">
                          <span>{t("triggers.descriptionLabel")}</span>
                          <input
                            value={card.description}
                            onChange={(event) =>
                              updateTrigger(card.id, (prev) => ({
                                ...prev,
                                description: event.target.value,
                              }))
                            }
                          />
                        </label>
                      </div>
                    </div>
                  ))}
                </div>
                <button type="button" className="secondary" onClick={addTrigger}>
                  {t("triggers.add")}
                </button>
              </SettingsCard>
            ) : null}
          </section>
        </div>
        <footer className="settings-actions">
          {message ? (
            <span className={`status-message ${message.type}`}>{message.text}</span>
          ) : null}
          <button type="button" onClick={handleSave}>
            {t("actions.save")}
          </button>
        </footer>
      </main>
    </>
  );
}

export default App;
