"use client";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { panelChoiceStyle } from "./settings-shared";

export function AppearancePanel() {
  const { locale, setLocale, setTheme, theme } = useUiPreferences();

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatUiMessage(locale, "settings.appearance.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "settings.appearance.sectionSubtitle")}
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
              <span style={{ color: "hsl(var(--muted-foreground))" }}>{description}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatUiMessage(locale, "settings.appearance.localeLabel")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "settings.appearance.sectionSubtitle")}
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
              <span style={{ color: "hsl(var(--muted-foreground))" }}>{description}</span>
            </button>
          ))}
        </div>
        <div className="app-inline-surface" style={{ display: "grid", gap: "0.45rem" }}>
          <div className="app-inline-row" style={{ marginBottom: 0 }}>
            <span>{formatUiMessage(locale, "settings.appearance.currentTheme")}</span>
            <strong>
              {{
                system: formatUiMessage(locale, "settings.appearance.theme.system"),
                light: formatUiMessage(locale, "settings.appearance.theme.light"),
                dark: formatUiMessage(locale, "settings.appearance.theme.dark"),
              }[theme]}
            </strong>
          </div>
          <div className="app-inline-row" style={{ marginBottom: 0 }}>
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

