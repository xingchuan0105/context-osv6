"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { type FormEvent, useState } from "react";
import { flushSync } from "react-dom";

import { GuestOnlyGate } from "@/components/auth-gates";
import ConsentCheckbox from "@/components/legal/ConsentCheckbox";
import { AuthFrame } from "@/components/page-frame";
import {
  PUBLISHED_PRIVACY_VERSION,
  PUBLISHED_TERMS_VERSION,
} from "@/lib/legal/versions";
import { register } from "@/lib/auth/client";
import { useAuth } from "@/lib/auth/context";
import { describeAuthError } from "@/lib/auth/errors";
import { formatUiMessage } from "@/lib/i18n/messages";
import { getSafeNextPath } from "@/lib/navigation/next-path";
import { useUiPreferences } from "@/lib/ui-preferences";

export default function RegisterPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { completeAuth } = useAuth();
  const { locale } = useUiPreferences();
  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [consented, setConsented] = useState(false);
  const nextPath = getSafeNextPath(searchParams.get("next"));

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const registerFailed = formatUiMessage(locale, "authRegisterFailed");

    if (!email.trim() || !password) {
      setError(formatUiMessage(locale, "authEmailAndPasswordRequired"));
      return;
    }

    if (password.length < 8) {
      setError(formatUiMessage(locale, "authPasswordMinLengthRequired"));
      return;
    }

    if (password !== confirmPassword) {
      setError(formatUiMessage(locale, "authPasswordMismatch"));
      return;
    }

    if (!consented) {
      setError("请先阅读并同意用户协议与隐私政策");
      return;
    }

    setLoading(true);
    setError("");

    try {
      const response = await register({
        email: email.trim(),
        password,
        full_name: fullName.trim() ? fullName.trim() : null,
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
      });

      if (!response.success || !response.data) {
        setError(response.error || registerFailed);
        return;
      }

      const payload = response.data;

      flushSync(() => {
        completeAuth(payload);
      });
      router.replace(nextPath ?? "/dashboard");
    } catch (submitError) {
      setError(describeAuthError(registerFailed, submitError, locale));
    } finally {
      setLoading(false);
    }
  }

  return (
    <GuestOnlyGate>
      <AuthFrame
        title={formatUiMessage(locale, "authCreateAccount")}
        subtitle={formatUiMessage(locale, "authRegisterSubtitle")}
      >
        <form onSubmit={handleSubmit} style={{ display: "grid", gap: "1rem" }}>
          <div>
            <label className="app-form-label" htmlFor="register-name">
              {formatUiMessage(locale, "authNameLabel")}
            </label>
            <input
              autoComplete="name"
              className="app-input"
              id="register-name"
              onChange={(event) => setFullName(event.target.value)}
              placeholder={formatUiMessage(locale, "authOptional")}
              type="text"
              value={fullName}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="register-email">
              {formatUiMessage(locale, "authEmailLabel")}
            </label>
            <input
              autoComplete="email"
              className="app-input"
              id="register-email"
              onChange={(event) => setEmail(event.target.value)}
              placeholder="name@example.com"
              type="email"
              value={email}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="register-password">
              {formatUiMessage(locale, "authPasswordLabel")}
            </label>
            <input
              autoComplete="new-password"
              className="app-input"
              id="register-password"
              onChange={(event) => setPassword(event.target.value)}
              placeholder={formatUiMessage(locale, "authPasswordMinLengthHint")}
              type="password"
              value={password}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="register-password-confirm">
              {formatUiMessage(locale, "authConfirmPasswordLabel")}
            </label>
            <input
              autoComplete="new-password"
              className="app-input"
              id="register-password-confirm"
              onChange={(event) => setConfirmPassword(event.target.value)}
              placeholder={formatUiMessage(locale, "authConfirmPasswordLabel")}
              type="password"
              value={confirmPassword}
            />
          </div>
          <ConsentCheckbox
            onConsentChange={setConsented}
            termsVersion={PUBLISHED_TERMS_VERSION}
            privacyVersion={PUBLISHED_PRIVACY_VERSION}
          />
          {error ? <p className="app-notice-banner">{error}</p> : null}
          <button className="app-button-primary app-button-block" disabled={loading} type="submit">
            {loading ? formatUiMessage(locale, "authCreatingAccount") : formatUiMessage(locale, "authCreateAccount")}
          </button>
        </form>
        <p className="app-form-footnote">
          {formatUiMessage(locale, "authHasAccount")}{" "}
          <Link className="app-link" href="/login">
            {formatUiMessage(locale, "authSignIn")}
          </Link>
        </p>
      </AuthFrame>
    </GuestOnlyGate>
  );
}
