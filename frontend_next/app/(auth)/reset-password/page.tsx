"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, useState } from "react";

import { AuthFrame } from "../../../components/page-frame";
import { sendResetCode } from "../../../lib/auth/client";
import { useAuth } from "../../../lib/auth/context";
import { describeAuthError } from "../../../lib/auth/errors";
import { formatUiMessage } from "../../../lib/i18n/messages";
import { storeResetEmail } from "../../../lib/auth/reset-state";
import { useUiPreferences } from "../../../lib/ui-preferences";

export default function ResetPasswordPage() {
  const router = useRouter();
  const { passwordResetEnabled } = useAuth();
  const { locale } = useUiPreferences();
  const [email, setEmail] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const sendResetFailed = formatUiMessage(locale, "authResetSendFailed");
    const trimmedEmail = email.trim();

    if (!trimmedEmail) {
      setError(formatUiMessage(locale, "authResetEmailRequired"));
      return;
    }

    setLoading(true);
    setError("");

    try {
      const response = await sendResetCode({
        email: trimmedEmail,
        lang: locale,
      });

      if (!response.success) {
        setError(response.error || sendResetFailed);
        return;
      }

      storeResetEmail(trimmedEmail);
      router.replace("/reset-password/verify");
    } catch (submitError) {
      setError(describeAuthError(sendResetFailed, submitError, locale));
    } finally {
      setLoading(false);
    }
  }

  if (!passwordResetEnabled) {
    return (
      <AuthFrame
        title={formatUiMessage(locale, "authResetRequestTitle")}
        subtitle={formatUiMessage(locale, "authResetUnavailable")}
      >
        <div style={{ display: "grid", gap: "1rem" }}>
          <p className="app-notice-banner">{formatUiMessage(locale, "authResetUnavailable")}</p>
          <Link className="app-link" href="/login">
            {formatUiMessage(locale, "authResetBackToLogin")}
          </Link>
        </div>
      </AuthFrame>
    );
  }

  return (
    <AuthFrame
      title={formatUiMessage(locale, "authResetRequestTitle")}
      subtitle={formatUiMessage(locale, "authResetRequestSubtitle")}
    >
      <form onSubmit={handleSubmit} style={{ display: "grid", gap: "1rem" }}>
        <div>
          <label className="app-form-label" htmlFor="reset-email">
            {formatUiMessage(locale, "authEmailLabel")}
          </label>
          <input
            autoComplete="email"
            className="app-input"
            id="reset-email"
            onChange={(event) => setEmail(event.target.value)}
            placeholder="name@example.com"
            type="email"
            value={email}
          />
        </div>
        {error ? <p className="app-notice-banner">{error}</p> : null}
        <div className="app-button-row">
          <button className="app-button-primary" disabled={loading} type="submit">
            {loading ? formatUiMessage(locale, "authResetSendSubmitting") : formatUiMessage(locale, "authResetSendSubmit")}
          </button>
          <Link className="app-link" href="/login">
            {formatUiMessage(locale, "authResetBackToLogin")}
          </Link>
        </div>
      </form>
    </AuthFrame>
  );
}
