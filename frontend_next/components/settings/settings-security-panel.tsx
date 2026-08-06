"use client";

import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { changePassword } from "../../lib/auth/client";
import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import shared from "./settings-ui-shared.module.css";

/**
 * Security panel (#10): change-password expands on demand; logout lives in account menu.
 */
export function SecurityPanel() {
  const router = useRouter();
  const { clearAuth, token } = useAuth();
  const { locale } = useUiPreferences();
  const [expanded, setExpanded] = useState(false);
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

  return (
    <section className={shared.section} data-testid="settings-security-panel">
      <section className={`app-inline-surface ${shared.section}`}>
        <div className={shared.headerText}>
          <h2 className={shared.flushTitle}>
            {formatUiMessage(locale, "settings.security.sectionTitle")}
          </h2>
          <p className={shared.mutedText}>
            {formatUiMessage(locale, "settings.security.sectionSubtitle")}
          </p>
        </div>

        {!expanded ? (
          <button
            className="app-button-secondary"
            data-testid="settings-change-password-expand"
            type="button"
            onClick={() => setExpanded(true)}
          >
            {formatUiMessage(locale, "settings.security.changePasswordAction")}
          </button>
        ) : (
          <form className={shared.formGrid} onSubmit={handleSubmit}>
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
              <button
                className="app-button-ghost"
                type="button"
                onClick={() => {
                  setExpanded(false);
                  setCurrentPassword("");
                  setNewPassword("");
                  setError("");
                }}
              >
                {formatUiMessage(locale, "appModal.close")}
              </button>
            </div>
          </form>
        )}
      </section>
    </section>
  );
}
