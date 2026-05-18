import type { TFunction } from "i18next";
import { CustomSelect } from "../CustomSelect";
import { HeroField, HeroInputField } from "../HeroField";
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
        <HeroField label={t("text.provider")}>
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
        </HeroField>
        <HeroInputField
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
        <HeroInputField
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
        <HeroInputField
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
        <HeroInputField
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
        <HeroField label={t("text.temperature")}>
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
        </HeroField>
        <HeroField label={t("text.maxOutputTokens")}>
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
        </HeroField>
        <HeroField label={t("text.topP")}>
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
        </HeroField>
      </SettingsCard>
    </>
  );
}
