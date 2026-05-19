import { Button } from "@heroui/react";
import { useTranslation } from "react-i18next";
import { languageOptions } from "../i18n/languages";

export function LanguageSwitcher() {
  const { t, i18n } = useTranslation();

  return (
    <div className="language-switcher">
      <span className="language-switcher-label">{t("language.label")}</span>
      {languageOptions.map((option) => (
        <Button
          key={option.code}
          type="button"
          className="language-switcher-button"
          variant={i18n.language === option.code ? "primary" : "secondary"}
          onPress={() => i18n.changeLanguage(option.code)}
          isDisabled={i18n.language === option.code}
        >
          {t(option.labelKey)}
        </Button>
      ))}
    </div>
  );
}
