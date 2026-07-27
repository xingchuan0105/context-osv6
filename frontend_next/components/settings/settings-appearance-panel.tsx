"use client";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { panelChoiceStyle } from "./settings-shared";
import styles from "./settings-appearance-panel.module.css";
import shared from "./settings-ui-shared.module.css";

export function AppearancePanel() {
  const { locale, setLocale, setTheme, theme } = useUiPreferences();

  return (
    <section className={shared.section}>
      <section className={`app-inline-surface ${shared.section}`}>
        <div className={shared.headerText}>
          <h2 className={shared.flushTitle}>
            {formatUiMessage(locale, "settings.appearance.sectionTitle")}
          </h2>
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.appearance.sectionSubtitle")}
          </p>
        </div>
        <div className={styles.choiceGrid}>
          {([
            [
              "system",
              formatUiMessage(locale, "settings.appearance.theme.system"),
              formatUiMessage(locale, "settings.appearance.themeDescription.system"),
            ],
            [
              "light",
              formatUiMessage(locale, "settings.appearance.theme.light"),
              formatUiMessage(locale, "settings.appearance.themeDescription.light"),
            ],
            [
              "dark",
              formatUiMessage(locale, "settings.appearance.theme.dark"),
              formatUiMessage(locale, "settings.appearance.themeDescription.dark"),
            ],
          ] as const).map(([value, title, description]) => (
            <button
              key={value}
              style={panelChoiceStyle(theme === value)}
              type="button"
              onClick={() => setTheme(value)}
            >
              <strong>{title}</strong>
              <span className={styles.mutedSpan}>{description}</span>
            </button>
          ))}
        </div>
      </section>

      <section className={`app-inline-surface ${shared.section}`}>
        <div className={shared.headerText}>
          <h2 className={shared.flushTitle}>
            {formatUiMessage(locale, "settings.appearance.localeLabel")}
          </h2>
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.appearance.localeSubtitle")}
          </p>
        </div>
        <div className={styles.choiceGrid}>
          {([
            [
              "zh-CN",
              formatUiMessage(locale, "workspaceLanguageChinese"),
              formatUiMessage(locale, "settings.appearance.localeDescription.zh-CN"),
            ],
            [
              "en",
              formatUiMessage(locale, "workspaceLanguageEnglish"),
              formatUiMessage(locale, "settings.appearance.localeDescription.en"),
            ],
          ] as const).map(([value, title, description]) => (
            <button
              key={value}
              style={panelChoiceStyle(locale === value)}
              type="button"
              onClick={() => setLocale(value)}
            >
              <strong>{title}</strong>
              <span className={styles.mutedSpan}>{description}</span>
            </button>
          ))}
        </div>
        <div className={`app-inline-surface ${styles.summaryCard}`}>
          <div className={`app-inline-row ${shared.summaryRow}`}>
            <span>{formatUiMessage(locale, "settings.appearance.currentTheme")}</span>
            <strong>
              {{
                system: formatUiMessage(locale, "settings.appearance.theme.system"),
                light: formatUiMessage(locale, "settings.appearance.theme.light"),
                dark: formatUiMessage(locale, "settings.appearance.theme.dark"),
              }[theme]}
            </strong>
          </div>
          <div className={`app-inline-row ${shared.summaryRow}`}>
            <span>
              {formatUiMessage(locale, "settings.appearance.currentLanguage")}
            </span>
            <strong>
              {locale === "zh-CN"
                ? formatUiMessage(locale, "workspaceLanguageChinese")
                : formatUiMessage(locale, "workspaceLanguageEnglish")}
            </strong>
          </div>
        </div>
      </section>
    </section>
  );
}
