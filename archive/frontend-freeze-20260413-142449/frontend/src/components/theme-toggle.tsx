'use client';

import { Monitor, Moon, Sun } from 'lucide-react';
import { useTheme } from 'next-themes';
import { useTranslation } from 'react-i18next';

export function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const { t } = useTranslation();

  const themes = [
    { value: 'light', icon: Sun, label: t('theme.light') },
    { value: 'dark', icon: Moon, label: t('theme.dark') },
    { value: 'system', icon: Monitor, label: t('theme.system') },
  ] as const;

  return (
    <div className="flex items-center gap-1 rounded-xl border border-border bg-background/55 p-1 shadow-[var(--shadow-sm)] backdrop-blur-sm">
      {themes.map(({ value, icon: Icon, label }) => (
        <button
          key={value}
          type="button"
          onClick={() => setTheme(value)}
          className={`flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm transition-colors ${
            theme === value ? 'bg-card text-foreground shadow-[var(--shadow-sm)]' : 'text-muted-foreground hover:text-foreground hover:bg-accent/70'
          }`}
        >
          <Icon className="h-4 w-4" />
          <span className="hidden sm:inline">{label}</span>
        </button>
      ))}
    </div>
  );
}
