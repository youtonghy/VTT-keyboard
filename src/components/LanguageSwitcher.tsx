import { useTranslation } from "react-i18next";
import { languageOptions } from "../i18n/languages";

export function LanguageSwitcher() {
  const { t, i18n } = useTranslation();

  return (
    <div className="language-switcher">
      <span>{t("language.label")}</span>
      {languageOptions.map((option) => (
        <button
          key={option.code}
          type="button"
          onClick={() => i18n.changeLanguage(option.code)}
          disabled={i18n.language === option.code}
        >
          {t(option.labelKey)}
        </button>
      ))}
    </div>
  );
}
