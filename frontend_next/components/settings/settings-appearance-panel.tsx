"use client";

import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { panelChoiceStyle } from "./settings-shared";

export function AppearancePanel() {
  const { locale, setLocale, setTheme, theme } = useUiPreferences();

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.appearance.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.appearance.sectionSubtitle")}
          </p>
        </div>
        <div
          style={{
            display: "grid",
            gap: "0.75rem",
            gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))",
          }}
        >
          {([
            [
              "system",
              formatSettingsShareMessage(locale, "settings.appearance.theme.system"),
              formatSettingsShareMessage(locale, "settings.appearance.themeDescription.system"),
            ],
            [
              "light",
              formatSettingsShareMessage(locale, "settings.appearance.theme.light"),
              formatSettingsShareMessage(locale, "settings.appearance.themeDescription.light"),
            ],
            [
              "dark",
              formatSettingsShareMessage(locale, "settings.appearance.theme.dark"),
              formatSettingsShareMessage(locale, "settings.appearance.themeDescription.dark"),
            ],
          ] as const).map(([value, title, description]) => (
            <button
              key={value}
              style={panelChoiceStyle(theme === value)}
              type="button"
              onClick={() => setTheme(value)}
            >
              <strong>{title}</strong>
              <span style={{ color: "hsl(var(--muted-foreground))" }}>{description}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.appearance.localeLabel")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.appearance.sectionSubtitle")}
          </p>
        </div>
        <div
          style={{
            display: "grid",
            gap: "0.75rem",
            gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))",
          }}
        >
          {([
            [
              "zh-CN",
              formatSettingsShareMessage(locale, "workspaceLanguageChinese"),
              formatSettingsShareMessage(locale, "settings.appearance.localeDescription.zh-CN"),
            ],
            [
              "en",
              formatSettingsShareMessage(locale, "workspaceLanguageEnglish"),
              formatSettingsShareMessage(locale, "settings.appearance.localeDescription.en"),
            ],
          ] as const).map(([value, title, description]) => (
            <button
              key={value}
              style={panelChoiceStyle(locale === value)}
              type="button"
              onClick={() => setLocale(value)}
            >
              <strong>{title}</strong>
              <span style={{ color: "hsl(var(--muted-foreground))" }}>{description}</span>
            </button>
          ))}
        </div>
        <div className="app-inline-surface" style={{ display: "grid", gap: "0.45rem" }}>
          <div className="app-inline-row" style={{ marginBottom: 0 }}>
            <span>{formatSettingsShareMessage(locale, "settings.appearance.currentTheme")}</span>
            <strong>
              {{
                system: formatSettingsShareMessage(locale, "settings.appearance.theme.system"),
                light: formatSettingsShareMessage(locale, "settings.appearance.theme.light"),
                dark: formatSettingsShareMessage(locale, "settings.appearance.theme.dark"),
              }[theme]}
            </strong>
          </div>
          <div className="app-inline-row" style={{ marginBottom: 0 }}>
            <span>
              {formatSettingsShareMessage(locale, "settings.appearance.currentLanguage")}
            </span>
            <strong>
              {locale === "zh-CN"
                ? formatSettingsShareMessage(locale, "workspaceLanguageChinese")
                : formatSettingsShareMessage(locale, "workspaceLanguageEnglish")}
            </strong>
          </div>
        </div>
      </section>
    </section>
  );
}

