import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import enTranslation from "./locales/en/translation.json";
import zhTranslation from "./locales/zh-CN/translation.json";

const resources = {
  en: { translation: enTranslation },
  "zh-CN": { translation: zhTranslation },
} as const;

const defaultLanguage =
  typeof navigator !== "undefined" &&
  navigator.language.toLowerCase().startsWith("zh")
    ? "zh-CN"
    : "en";

i18n.use(initReactI18next).init({
  resources,
  fallbackLng: "en",
  supportedLngs: ["en", "zh-CN"],
  lng: defaultLanguage,
  interpolation: {
    escapeValue: false,
  },
  react: {
    useSuspense: false,
  },
});

export default i18n;
