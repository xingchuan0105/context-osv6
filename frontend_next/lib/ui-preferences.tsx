"use client";

import {
  createContext,
  type ReactNode,
  startTransition,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useRouter } from "next/navigation";
import { DEFAULT_LOCALE, LOCALE_COOKIE_NAME, normalizeLocale, type UiLocale } from "./i18n/config";

export type UiTheme = "system" | "light" | "dark";
export type { UiLocale } from "./i18n/config";

const THEME_STORAGE_KEY = "avrag.ui.theme.v1";
const LOCALE_STORAGE_KEY = "avrag.ui.locale.v1";

type UiPreferencesContextValue = {
  theme: UiTheme;
  locale: UiLocale;
  setTheme: (theme: UiTheme) => void;
  setLocale: (locale: UiLocale) => void;
};

const UiPreferencesContext = createContext<UiPreferencesContextValue | null>(null);

function normalizeTheme(value: string | null) {
  if (value === "light" || value === "dark" || value === "system") {
    return value;
  }

  return "system";
}

function readStoredTheme() {
  if (typeof window === "undefined") {
    return "system" as UiTheme;
  }

  return normalizeTheme(window.localStorage.getItem(THEME_STORAGE_KEY));
}

function writeLocaleCookie(locale: UiLocale) {
  if (typeof document === "undefined") {
    return;
  }

  document.cookie = `${LOCALE_COOKIE_NAME}=${encodeURIComponent(locale)}; Path=/; Max-Age=31536000; SameSite=Lax`;
}

function applyDocumentPreferences(theme: UiTheme, locale: UiLocale) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.lang = locale;

  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
    return;
  }

  document.documentElement.setAttribute("data-theme", theme);
}

export function chooseUiLabel(locale: UiLocale, zh: string, en: string) {
  return locale === "zh-CN" ? zh : en;
}

export function UiPreferencesProvider({
  children,
  initialLocale = DEFAULT_LOCALE,
}: {
  children: ReactNode;
  initialLocale?: UiLocale;
}) {
  const router = useRouter();
  const [theme, setTheme] = useState<UiTheme>(readStoredTheme);
  const [locale, setLocaleState] = useState<UiLocale>(normalizeLocale(initialLocale));

  useEffect(() => {
    setLocaleState(normalizeLocale(initialLocale));
  }, [initialLocale]);

  useEffect(() => {
    applyDocumentPreferences(theme, locale);

    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    writeLocaleCookie(locale);
  }, [locale, theme]);

  function setLocale(nextLocale: UiLocale) {
    if (nextLocale === locale) {
      return;
    }

    setLocaleState(nextLocale);

    startTransition(() => {
      router.refresh();
    });
  }

  const value = useMemo<UiPreferencesContextValue>(
    () => ({
      theme,
      locale,
      setTheme,
      setLocale,
    }),
    [locale, theme],
  );

  return <UiPreferencesContext.Provider value={value}>{children}</UiPreferencesContext.Provider>;
}

export function useUiPreferences() {
  const context = useContext(UiPreferencesContext);

  if (!context) {
    throw new Error("useUiPreferences must be used inside UiPreferencesProvider");
  }

  return context;
}
