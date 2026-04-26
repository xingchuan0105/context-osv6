"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, useState } from "react";

import { AuthFrame } from "../../../../components/page-frame";
import { verifyResetCode } from "../../../../lib/auth/client";
import { useAuth } from "../../../../lib/auth/context";
import { describeAuthError } from "../../../../lib/auth/errors";
import { formatUiMessage } from "../../../../lib/i18n/messages";
import { readResetEmail, storeResetTicket } from "../../../../lib/auth/reset-state";
import { useUiPreferences } from "../../../../lib/ui-preferences";

export default function VerifyResetPage() {
  const router = useRouter();
  const { passwordResetEnabled } = useAuth();
  const { locale } = useUiPreferences();
  const [email, setEmail] = useState(() => readResetEmail() ?? "");
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const verifyResetFailed = formatUiMessage(locale, "authResetVerifyFailed");
    const trimmedEmail = email.trim();

    if (!trimmedEmail || !code.trim()) {
      setError(formatUiMessage(locale, "authEmailAndCodeRequired"));
      return;
    }

    setLoading(true);
    setError("");

    try {
      const response = await verifyResetCode({
        email: trimmedEmail,
        code: code.trim(),
      });

      if (!response.success || !response.data) {
        setError(response.error || verifyResetFailed);
        return;
      }

      storeResetTicket(response.data.reset_ticket ?? "");
      router.replace("/reset-password/confirm");
    } catch (submitError) {
      setError(describeAuthError(verifyResetFailed, submitError, locale));
    } finally {
      setLoading(false);
    }
  }

  if (!passwordResetEnabled) {
    return (
      <AuthFrame
        title={formatUiMessage(locale, "authResetVerifyTitle")}
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
      title={formatUiMessage(locale, "authResetVerifyTitle")}
      subtitle={formatUiMessage(locale, "authResetVerifySubtitle")}
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
        <div>
          <label className="app-form-label" htmlFor="reset-code">
            {formatUiMessage(locale, "authResetCodeLabel")}
          </label>
          <input
            autoComplete="one-time-code"
            className="app-input"
            id="reset-code"
            onChange={(event) => setCode(event.target.value)}
            placeholder={formatUiMessage(locale, "authResetCodeHint")}
            value={code}
          />
        </div>
        {error ? <p className="app-notice-banner">{error}</p> : null}
        <div className="app-button-row">
          <button className="app-button-primary" disabled={loading} type="submit">
            {loading ? formatUiMessage(locale, "authResetVerifySubmitting") : formatUiMessage(locale, "authResetVerifySubmit")}
          </button>
          <Link className="app-link" href="/reset-password">
            {formatUiMessage(locale, "authResetBackToPrevious")}
          </Link>
        </div>
      </form>
    </AuthFrame>
  );
}
