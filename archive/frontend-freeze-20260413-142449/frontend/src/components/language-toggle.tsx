'use client';

import { Languages } from 'lucide-react';
import i18n from '@/lib/i18n';
import { useTranslation } from 'react-i18next';

export function LanguageToggle() {
  const { t } = useTranslation();

  const changeLanguage = (lng: 'zh' | 'en') => {
    void i18n.changeLanguage(lng);
  };

  const current = i18n.resolvedLanguage?.startsWith('en') ? 'en' : 'zh';

  return (
    <div className="space-y-2">
      <div className="text-sm font-medium text-muted-foreground">{t('settings.language')}</div>
      <div className="flex items-center gap-1 rounded-xl border border-border bg-background/55 p-1 shadow-[var(--shadow-sm)] backdrop-blur-sm">
        <button
          type="button"
          onClick={() => changeLanguage('zh')}
          className={`flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm transition-colors ${
            current === 'zh' ? 'bg-card text-foreground shadow-[var(--shadow-sm)]' : 'text-muted-foreground hover:text-foreground hover:bg-accent/70'
          }`}
          aria-label={t('language.zh')}
        >
          <Languages className="h-4 w-4" />
          <span>{t('language.zh')}</span>
        </button>
        <button
          type="button"
          onClick={() => changeLanguage('en')}
          className={`flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm transition-colors ${
            current === 'en' ? 'bg-card text-foreground shadow-[var(--shadow-sm)]' : 'text-muted-foreground hover:text-foreground hover:bg-accent/70'
          }`}
          aria-label={t('language.en')}
        >
          <Languages className="h-4 w-4" />
          <span>{t('language.en')}</span>
        </button>
      </div>
    </div>
  );
}
