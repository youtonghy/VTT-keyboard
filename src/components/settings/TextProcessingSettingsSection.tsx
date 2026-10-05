import type { TFunction } from "i18next";
import { CustomSelect } from "../CustomSelect";
import { Field, InputField } from "../FormFields";
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
        <Field label={t("text.provider")}>
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
        </Field>
        <InputField
          label={t("openai.apiBase")}
          value={draft.textProcessing.openai.apiBase}
          onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    apiBase: value,
                  },
                },
              }))
          }
        />
        <InputField
          label={t("openai.apiKey")}
          value={draft.textProcessing.openai.apiKey}
          type="password"
          onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    apiKey: value,
                  },
                },
              }))
          }
        />
      </SettingsCard>

      <SettingsCard title={t("text.openaiTitle")}>
        <InputField
          label={t("text.model")}
          value={draft.textProcessing.openai.model}
          onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    model: value,
                  },
                },
              }))
          }
        />
        <InputField
          label={t("text.instructions")}
          value={draft.textProcessing.openai.instructions}
          onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    instructions: value,
                  },
                },
              }))
          }
        />
        <Field label={t("text.thinking")}>
          <CustomSelect
            value={draft.textProcessing.openai.thinking}
            onChange={(value) =>
              updateDraft((prev) => ({
                ...prev,
                textProcessing: {
                  ...prev.textProcessing,
                  openai: {
                    ...prev.textProcessing.openai,
                    thinking: value as Settings["textProcessing"]["openai"]["thinking"],
                  },
                },
              }))
            }
            options={[
              { value: "default", label: t("text.thinkingDefault") },
              { value: "on", label: t("text.thinkingOn") },
              { value: "off", label: t("text.thinkingOff") },
            ]}
          />
        </Field>
        <Field label={t("text.temperature")}>
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
        </Field>
        <Field label={t("text.maxOutputTokens")}>
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
        </Field>
        <Field label={t("text.topP")}>
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
        </Field>
      </SettingsCard>
    </>
  );
}
