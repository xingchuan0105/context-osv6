"use client";

import { type KeyboardEvent, useEffect, useState, type ReactNode } from "react";

import { cloudLogin, getCloudSession, isCloudGateBypassed } from "@/lib/desktop/tauri-cloud";
import { openInBrowser } from "@/lib/desktop/tauri-license";
import { formatUiMessage } from "@/lib/i18n/messages";
import { isTauri } from "@/lib/runtime/tauri-ipc";
import { APP_PATHS, appAbsoluteUrl } from "@/lib/site-map";
import { useUiPreferences } from "@/lib/ui-preferences";

/**
 * Desktop cloud login gate (2026-08-15 wave, W3): without a cloud session the
 * app shell does not render — official models (走余额) need the relay
 * credentials that only a cloud login mints. The login itself runs Rust-side
 * (reqwest), so this card never touches WebView fetch / CORS.
 *
 * Mounted between ClientLicenseGate and ClientLocalSessionBootstrap: login
 * needs no local stack; the bootstrap afterwards brings the stack/product up
 * with the relay env already in client.env.
 */
export function CloudLoginGate({ children }: { children: ReactNode }) {
  const { locale } = useUiPreferences();
  const [phase, setPhase] = useState<"checking" | "login" | "ready">("checking");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      setPhase("ready");
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        // scripts/desktop-e2e launches the shell with the bypass env set — no
        // real cloud account exists in that environment.
        if (await isCloudGateBypassed()) {
          return "ready" as const;
        }
        const session = await getCloudSession();
        return session.logged_in ? ("ready" as const) : ("login" as const);
      } catch {
        return "login" as const;
      }
    })().then((next) => {
      if (!cancelled) {
        setPhase(next);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSubmit() {
    if (submitting) {
      return;
    }
    setSubmitting(true);
    setError("");
    try {
      await cloudLogin(email.trim(), password);
      setPhase("ready");
    } catch (loginError) {
      const fallback = formatUiMessage(locale, "desktop.cloudLoginFailed");
      setError(
        loginError instanceof Error && loginError.message.trim()
          ? loginError.message
          : fallback,
      );
    } finally {
      setSubmitting(false);
    }
  }

  function handleFieldKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") {
      return;
    }
    event.preventDefault();
    void handleSubmit();
  }

  if (phase === "ready") {
    return <>{children}</>;
  }

  if (phase === "checking") {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "desktop.cloudLoginChecking")}
          </p>
        </section>
      </main>
    );
  }

  return (
    <main className="app-auth-shell">
      <section className="app-surface-card" style={{ maxWidth: "28rem" }}>
        <h1 className="app-auth-title" style={{ marginBottom: "0.5rem" }}>
          {formatUiMessage(locale, "desktop.cloudLoginTitle")}
        </h1>
        <p
          style={{
            margin: "0 0 1rem",
            fontSize: "0.85rem",
            color: "hsl(var(--muted-foreground))",
          }}
        >
          {formatUiMessage(locale, "desktop.cloudLoginSubtitle")}
        </p>
        <div style={{ display: "grid", gap: "1rem" }}>
          <div>
            <label className="app-form-label" htmlFor="cloud-login-email">
              {formatUiMessage(locale, "desktop.cloudLoginEmail")}
            </label>
            <input
              autoComplete="email"
              className="app-input"
              id="cloud-login-email"
              onChange={(event) => setEmail(event.target.value)}
              onKeyDown={handleFieldKeyDown}
              placeholder="name@example.com"
              type="email"
              value={email}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="cloud-login-password">
              {formatUiMessage(locale, "desktop.cloudLoginPassword")}
            </label>
            <input
              autoComplete="current-password"
              className="app-input"
              id="cloud-login-password"
              onChange={(event) => setPassword(event.target.value)}
              onKeyDown={handleFieldKeyDown}
              type="password"
              value={password}
            />
          </div>
          {error ? <p className="app-notice-banner">{error}</p> : null}
          <button
            className="app-button-primary app-button-block"
            disabled={submitting || !email.trim() || !password}
            type="button"
            onClick={() => void handleSubmit()}
          >
            {submitting
              ? formatUiMessage(locale, "desktop.cloudLoginSubmitting")
              : formatUiMessage(locale, "desktop.cloudLoginSubmit")}
          </button>
        </div>
        <p className="app-form-footnote">
          {formatUiMessage(locale, "desktop.cloudLoginNoAccount")}{" "}
          <button
            className="app-button-ghost"
            type="button"
            onClick={() => void openInBrowser(appAbsoluteUrl(APP_PATHS.register))}
          >
            {formatUiMessage(locale, "desktop.cloudLoginRegister")}
          </button>
        </p>
        <p className="app-form-footnote">
          {formatUiMessage(locale, "desktop.cloudLoginByokHint")}
        </p>
      </section>
    </main>
  );
}
