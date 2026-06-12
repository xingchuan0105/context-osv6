"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { type KeyboardEvent, useEffect, useState } from "react";
import { flushSync } from "react-dom";

import { GuestOnlyGate } from "@/components/auth-gates";
import { AuthFrame } from "@/components/page-frame";
import { login } from "@/lib/auth/client";
import { useAuth } from "@/lib/auth/context";
import { describeAuthError } from "@/lib/auth/errors";
import { formatUiMessage } from "@/lib/i18n/messages";
import { getSafeNextPath } from "@/lib/navigation/next-path";
import { useUiPreferences } from "@/lib/ui-preferences";

export default function LoginPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { completeAuth, passwordResetEnabled } = useAuth();
  const { locale } = useUiPreferences();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [interactive, setInteractive] = useState(false);
  const nextPath = getSafeNextPath(searchParams.get("next"));

  useEffect(() => {
    setInteractive(true);
  }, []);

  async function handleSubmit() {
    const loginFailed = formatUiMessage(locale, "authLoginFailed");

    if (!interactive || loading) {
      return;
    }

    if (!email.trim() || !password) {
      setError(formatUiMessage(locale, "authEmailAndPasswordRequired"));
      return;
    }

    setLoading(true);
    setError("");

    try {
      const response = await login({
        email: email.trim(),
        password,
      });

      if (!response.success || !response.data) {
        setError(response.error || loginFailed);
        return;
      }

      const payload = response.data;

      flushSync(() => {
        completeAuth(payload);
      });
      router.replace(nextPath ?? "/dashboard");
    } catch (submitError) {
      setError(describeAuthError(loginFailed, submitError, locale));
    } finally {
      setLoading(false);
    }
  }

  function handleFieldKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") {
      return;
    }

    event.preventDefault();
    void handleSubmit();
  }

  return (
    <GuestOnlyGate>
      <AuthFrame
        title={formatUiMessage(locale, "authLoginTitle")}
        subtitle={formatUiMessage(locale, "authLoginSubtitle")}
      >
        <div style={{ display: "grid", gap: "1rem" }}>
          <div>
            <label className="app-form-label" htmlFor="login-email">
              {formatUiMessage(locale, "authEmailLabel")}
            </label>
            <input
              autoComplete="email"
              className="app-input"
              id="login-email"
              onChange={(event) => setEmail(event.target.value)}
              onKeyDown={handleFieldKeyDown}
              placeholder="name@example.com"
              type="email"
              value={email}
            />
          </div>
          <div>
            <div className="app-inline-row">
              <label className="app-form-label" htmlFor="login-password" style={{ marginBottom: 0 }}>
                {formatUiMessage(locale, "authPasswordLabel")}
              </label>
              {passwordResetEnabled ? (
                <Link className="app-link app-link-muted" href="/reset-password">
                  {formatUiMessage(locale, "authForgotPassword")}
                </Link>
              ) : null}
            </div>
            <input
              autoComplete="current-password"
              className="app-input"
              id="login-password"
              onChange={(event) => setPassword(event.target.value)}
              onKeyDown={handleFieldKeyDown}
              placeholder={formatUiMessage(locale, "authPasswordMinLengthHint")}
              type="password"
              value={password}
            />
          </div>
          {error ? <p className="app-notice-banner">{error}</p> : null}
          <button
            className="app-button-primary app-button-block"
            disabled={loading || !interactive}
            type="button"
            onClick={() => void handleSubmit()}
          >
            {loading ? formatUiMessage(locale, "authLoginSubmitting") : formatUiMessage(locale, "authLoginSubmit")}
          </button>
        </div>
        <p className="app-form-footnote">
          {formatUiMessage(locale, "authNeedsAccount")}{" "}
          <Link className="app-link" href="/register">
            {formatUiMessage(locale, "authSignUp")}
          </Link>
        </p>
      </AuthFrame>
    </GuestOnlyGate>
  );
}
