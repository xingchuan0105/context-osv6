"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { changePassword } from "../../lib/auth/client";
import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import { useUiPreferences } from "../../lib/ui-preferences";

export function SecurityPanel() {
  const router = useRouter();
  const { clearAuth, logout, passwordResetEnabled, token, user } = useAuth();
  const { locale, theme } = useUiPreferences();
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!token) {
      setError(formatSettingsShareMessage(locale, "settings.security.notAuthenticated"));
      return;
    }

    if (!currentPassword.trim() || !newPassword.trim()) {
      setError(formatSettingsShareMessage(locale, "settings.security.missingPassword"));
      return;
    }

    setLoading(true);
    setError("");

    try {
      await changePassword(token, {
        old_password: currentPassword,
        new_password: newPassword,
      });
      clearAuth();
      router.replace("/login");
    } catch (submitError) {
      setError(
        describeAuthError(
          formatSettingsShareMessage(locale, "settings.security.failed"),
          submitError,
        ),
      );
    } finally {
      setLoading(false);
    }
  }

  async function handleLogout() {
    await logout();
    router.replace("/login");
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "grid", gap: "0.35rem" }}>
          <h2 style={{ margin: 0 }}>
            {formatSettingsShareMessage(locale, "settings.security.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatSettingsShareMessage(locale, "settings.security.sectionSubtitle")}
          </p>
        </div>
        <form style={{ display: "grid", gap: "1rem" }} onSubmit={handleSubmit}>
          <div>
            <label className="app-form-label" htmlFor="settings-current-password">
              {formatSettingsShareMessage(locale, "settings.security.currentPasswordLabel")}
            </label>
            <input
              autoComplete="current-password"
              className="app-input"
              id="settings-current-password"
              type="password"
              value={currentPassword}
              onChange={(event) => setCurrentPassword(event.target.value)}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="settings-new-password">
              {formatSettingsShareMessage(locale, "settings.security.newPasswordLabel")}
            </label>
            <input
              autoComplete="new-password"
              className="app-input"
              id="settings-new-password"
              type="password"
              value={newPassword}
              onChange={(event) => setNewPassword(event.target.value)}
            />
          </div>
          {error ? <p className="app-notice-banner">{error}</p> : null}
          <div className="app-button-row">
            <button className="app-button-primary" disabled={loading} type="submit">
              {loading
                ? formatSettingsShareMessage(locale, "settings.security.updating")
                : formatSettingsShareMessage(locale, "settings.security.changePasswordAction")}
            </button>
            {passwordResetEnabled ? (
              <Link className="app-button-secondary" href="/reset-password">
                {formatSettingsShareMessage(locale, "settings.security.resetPasswordAction")}
              </Link>
            ) : null}
            <button className="app-button-ghost" type="button" onClick={() => void handleLogout()}>
              {formatSettingsShareMessage(locale, "workspaceLogout")}
            </button>
          </div>
        </form>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatSettingsShareMessage(locale, "settings.security.currentSessionTitle")}
        </h3>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatSettingsShareMessage(locale, "settings.security.signedInAs")}</span>
          <strong>
            {user?.email ??
              formatSettingsShareMessage(locale, "settings.security.unknownAccount")}
          </strong>
        </div>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatSettingsShareMessage(locale, "settings.appearance.currentLanguage")}</span>
          <strong>
            {locale === "zh-CN"
              ? formatSettingsShareMessage(locale, "workspaceLanguageChinese")
              : formatSettingsShareMessage(locale, "workspaceLanguageEnglish")}
          </strong>
        </div>
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
      </section>
    </section>
  );
}

