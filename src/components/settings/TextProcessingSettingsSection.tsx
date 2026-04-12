import type { TFunction } from "i18next";
import { CustomSelect } from "../CustomSelect";
import { NumberWheelInput } from "../NumberWheelInput";
import { SettingsCard } from "../SettingsCard";
import type { Settings } from "../../types/settings";

interface TextProcessingSettingsSectionProps {
  draft: Settings;
  t: TFunction;
  updateDraft: (updater: (prev: Settings) => Settings) => void;
}

export function TextProcessingSettingsSection({
  draft,
  t,
  updateDraft,
}: TextProcessingSettingsSectionProps) {
  return (
    <>
      <SettingsCard title={t("text.title")} description={t("text.description")}>
        <label className="field">
          <span>{t("text.provider")}</span>
          <CustomSelect
            value={draft.textProcessing.provider}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  provider: value as Settings["textProcessing"]["provider"],
                },
              }))
            }
            options={[{ value: "openai", label: "OpenAI" }]}
          />
        </label>
        <label className="field">
          <span>{t("openai.apiBase")}</span>
          <input
            value={draft.textProcessing.openai.apiBase}
            onChange={(event) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    apiBase: event.target.value,
                  },
                },
              }))
            }
          />
        </label>
        <label className="field">
          <span>{t("openai.apiKey")}</span>
          <input
            type="password"
            value={draft.textProcessing.openai.apiKey}
            onChange={(event) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    apiKey: event.target.value,
                  },
                },
              }))
            }
          />
        </label>
      </SettingsCard>

      <SettingsCard title={t("text.openaiTitle")}>
        <label className="field">
          <span>{t("text.model")}</span>
          <input
            value={draft.textProcessing.openai.model}
            onChange={(event) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    model: event.target.value,
                  },
                },
              }))
            }
          />
        </label>
        <label className="field">
          <span>{t("text.instructions")}</span>
          <input
            value={draft.textProcessing.openai.instructions}
            onChange={(event) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    instructions: event.target.value,
                  },
                },
              }))
            }
          />
        </label>
        <label className="field">
          <span>{t("text.temperature")}</span>
          <NumberWheelInput
            step={0.1}
            value={draft.textProcessing.openai.temperature}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    temperature: value,
                  },
                },
              }))
            }
          />
        </label>
        <label className="field">
          <span>{t("text.maxOutputTokens")}</span>
          <NumberWheelInput
            min={1}
            value={draft.textProcessing.openai.maxOutputTokens}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    maxOutputTokens: value,
                  },
                },
              }))
            }
          />
        </label>
        <label className="field">
          <span>{t("text.topP")}</span>
          <NumberWheelInput
            step={0.1}
            value={draft.textProcessing.openai.topP}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    topP: value,
                  },
                },
              }))
            }
          />
        </label>
      </SettingsCard>
    </>
  );
}
