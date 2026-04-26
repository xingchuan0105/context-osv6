"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, useState } from "react";

import { AuthFrame } from "../../../../components/page-frame";
import { confirmResetPassword } from "../../../../lib/auth/client";
import { useAuth } from "../../../../lib/auth/context";
import { describeAuthError } from "../../../../lib/auth/errors";
import { formatUiMessage } from "../../../../lib/i18n/messages";
import { clearResetFlowState, readResetTicket } from "../../../../lib/auth/reset-state";
import { useUiPreferences } from "../../../../lib/ui-preferences";

export default function ConfirmResetPage() {
  const router = useRouter();
  const { passwordResetEnabled } = useAuth();
  const { locale } = useUiPreferences();
  const [resetTicket] = useState(() => readResetTicket() ?? "");
  const [newPassword, setNewPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const confirmResetFailed = formatUiMessage(locale, "authResetConfirmFailed");

    if (!newPassword) {
      setError(formatUiMessage(locale, "authNewPasswordRequired"));
      return;
    }

    setLoading(true);
    setError("");

    try {
      const response = await confirmResetPassword({
        reset_ticket: resetTicket,
        new_password: newPassword,
      });

      if (!response.success) {
        setError(response.error || confirmResetFailed);
        return;
      }

      clearResetFlowState();
      router.replace("/login");
    } catch (submitError) {
      setError(describeAuthError(confirmResetFailed, submitError, locale));
    } finally {
      setLoading(false);
    }
  }

  if (!passwordResetEnabled || !resetTicket) {
    return (
      <AuthFrame
        title={formatUiMessage(locale, "authResetConfirmTitle")}
        subtitle={formatUiMessage(locale, "authResetConfirmUnavailable")}
      >
        <div style={{ display: "grid", gap: "1rem" }}>
          <p className="app-notice-banner">{formatUiMessage(locale, "authResetConfirmUnavailable")}</p>
          <Link className="app-link" href="/reset-password">
            {formatUiMessage(locale, "authResetBackToStart")}
          </Link>
        </div>
      </AuthFrame>
    );
  }

  return (
    <AuthFrame
      title={formatUiMessage(locale, "authResetConfirmTitle")}
      subtitle={formatUiMessage(locale, "authResetConfirmSubtitle")}
    >
      <form onSubmit={handleSubmit} style={{ display: "grid", gap: "1rem" }}>
        <div>
          <label className="app-form-label" htmlFor="new-password">
            {formatUiMessage(locale, "authNewPasswordLabel")}
          </label>
          <input
            autoComplete="new-password"
            className="app-input"
            id="new-password"
            onChange={(event) => setNewPassword(event.target.value)}
            placeholder={formatUiMessage(locale, "authPasswordMinLengthHint")}
            type="password"
            value={newPassword}
          />
        </div>
        {error ? <p className="app-notice-banner">{error}</p> : null}
        <div className="app-button-row">
          <button className="app-button-primary" disabled={loading} type="submit">
            {loading ? formatUiMessage(locale, "authResetConfirmSubmitting") : formatUiMessage(locale, "authResetConfirmSubmit")}
          </button>
        </div>
      </form>
    </AuthFrame>
  );
}
