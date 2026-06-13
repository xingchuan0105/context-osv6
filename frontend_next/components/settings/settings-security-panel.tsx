"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { changePassword } from "../../lib/auth/client";
import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
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
      setError(formatUiMessage(locale, "settings.security.notAuthenticated"));
      return;
    }

    if (!currentPassword.trim() || !newPassword.trim()) {
      setError(formatUiMessage(locale, "settings.security.missingPassword"));
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
          formatUiMessage(locale, "settings.security.failed"),
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
            {formatUiMessage(locale, "settings.security.sectionTitle")}
          </h2>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "settings.security.sectionSubtitle")}
          </p>
        </div>
        <form style={{ display: "grid", gap: "1rem" }} onSubmit={handleSubmit}>
          <div>
            <label className="app-form-label" htmlFor="settings-current-password">
              {formatUiMessage(locale, "settings.security.currentPasswordLabel")}
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
              {formatUiMessage(locale, "settings.security.newPasswordLabel")}
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
                ? formatUiMessage(locale, "settings.security.updating")
                : formatUiMessage(locale, "settings.security.changePasswordAction")}
            </button>
            {passwordResetEnabled ? (
              <Link className="app-button-secondary" href="/reset-password">
                {formatUiMessage(locale, "settings.security.resetPasswordAction")}
              </Link>
            ) : null}
            <button className="app-button-ghost" type="button" onClick={() => void handleLogout()}>
              {formatUiMessage(locale, "workspaceLogout")}
            </button>
          </div>
        </form>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatUiMessage(locale, "settings.security.currentSessionTitle")}
        </h3>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatUiMessage(locale, "settings.security.signedInAs")}</span>
          <strong>
            {user?.email ??
              formatUiMessage(locale, "settings.security.unknownAccount")}
          </strong>
        </div>
        <div className="app-inline-row" style={{ marginBottom: 0 }}>
          <span>{formatUiMessage(locale, "settings.appearance.currentLanguage")}</span>
          <strong>
            {locale === "zh-CN"
              ? formatUiMessage(locale, "workspaceLanguageChinese")
              : formatUiMessage(locale, "workspaceLanguageEnglish")}
          </strong>
        </div>
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
      </section>
    </section>
  );
}

