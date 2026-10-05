import { useTranslation } from "react-i18next";
import { languageOptions } from "../i18n/languages";
import { SegmentedControl } from "./SegmentedControl";

export function LanguageSwitcher() {
  const { t, i18n } = useTranslation();

  return (
    <SegmentedControl
      value={i18n.language}
      onChange={(code) => void i18n.changeLanguage(code)}
      options={languageOptions.map((option) => ({
        value: option.code,
        label: t(option.labelKey),
      }))}
    />
  );
}
