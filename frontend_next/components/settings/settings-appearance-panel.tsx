"use client";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./settings-appearance-panel.module.css";
import shared from "./settings-ui-shared.module.css";

/**
 * Preferences panel (#9): theme + locale as row dropdowns (no big choice cards).
 */
export function AppearancePanel() {
  const { locale, setLocale, setTheme, theme } = useUiPreferences();

  return (
    <section className={shared.section} data-testid="settings-preferences-panel">
      <div className={shared.headerText}>
        <h2 className={shared.flushTitle}>
          {formatUiMessage(locale, "settings.appearance.sectionTitle")}
        </h2>
        <p className={shared.mutedText}>
          {formatUiMessage(locale, "settings.appearance.sectionSubtitle")}
        </p>
      </div>

      <div className={`app-inline-surface ${styles.rowCard}`}>
        <label className={styles.row} htmlFor="settings-theme-select">
          <span className={styles.rowLabel}>
            {formatUiMessage(locale, "settings.appearance.themeLabel")}
          </span>
          <select
            className="app-input"
            data-testid="settings-theme-select"
            id="settings-theme-select"
            value={theme}
            onChange={(event) =>
              setTheme(event.target.value as "system" | "light" | "dark")
            }
          >
            <option value="system">
              {formatUiMessage(locale, "settings.appearance.theme.system")}
            </option>
            <option value="light">
              {formatUiMessage(locale, "settings.appearance.theme.light")}
            </option>
            <option value="dark">
              {formatUiMessage(locale, "settings.appearance.theme.dark")}
            </option>
          </select>
        </label>
        <p className={styles.rowHint}>
          {
            {
              system: formatUiMessage(locale, "settings.appearance.themeDescription.system"),
              light: formatUiMessage(locale, "settings.appearance.themeDescription.light"),
              dark: formatUiMessage(locale, "settings.appearance.themeDescription.dark"),
            }[theme]
          }
        </p>
      </div>

      <div className={`app-inline-surface ${styles.rowCard}`}>
        <label className={styles.row} htmlFor="settings-locale-select">
          <span className={styles.rowLabel}>
            {formatUiMessage(locale, "settings.appearance.localeLabel")}
          </span>
          <select
            className="app-input"
            data-testid="settings-locale-select"
            id="settings-locale-select"
            value={locale}
            onChange={(event) => setLocale(event.target.value as "zh-CN" | "en")}
          >
            <option value="zh-CN">
              {formatUiMessage(locale, "workspaceLanguageChinese")}
            </option>
            <option value="en">
              {formatUiMessage(locale, "workspaceLanguageEnglish")}
            </option>
          </select>
        </label>
        <p className={styles.rowHint}>
          {formatUiMessage(locale, "settings.appearance.localeSubtitle")}
        </p>
      </div>
    </section>
  );
}
