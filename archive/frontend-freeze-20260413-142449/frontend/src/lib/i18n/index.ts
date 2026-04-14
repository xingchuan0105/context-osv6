import i18n from 'i18next';
import LanguageDetector from 'i18next-browser-languagedetector';
import { initReactI18next } from 'react-i18next';
import en from './en';
import zh from './zh';

if (!i18n.isInitialized) {
  i18n
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
      resources: {
        zh: { translation: zh },
        en: { translation: en },
      },
      lng: 'zh',
      fallbackLng: 'zh',
      supportedLngs: ['zh', 'en'],
      load: 'languageOnly',
      interpolation: {
        escapeValue: false,
      },
      detection: {
        order: ['localStorage'],
        caches: ['localStorage'],
        lookupLocalStorage: 'context-os-language',
      },
    });
}

export default i18n;
