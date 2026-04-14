'use client';

import { useEffect } from 'react';
import { I18nextProvider } from 'react-i18next';
import i18n from '@/lib/i18n';

export function LanguageProvider({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    const applyLang = (lng: string) => {
      document.documentElement.lang = lng.startsWith('en') ? 'en-US' : 'zh-CN';
    };

    applyLang(i18n.resolvedLanguage || i18n.language);
    i18n.on('languageChanged', applyLang);
    return () => {
      i18n.off('languageChanged', applyLang);
    };
  }, []);

  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
}
