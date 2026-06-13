"use client";

import { type ReactNode, useEffect, useState } from "react";

import ConsentCheckbox from "@/components/legal/ConsentCheckbox";
import { describeAuthError } from "@/lib/auth/errors";
import { useAuth } from "@/lib/auth/context";
import { fetchLegalStatus, recordLegalAcceptance } from "@/lib/legal/client";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

type GateState =
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "blocked"; error: string };

export function LegalReacceptanceGate({ children }: { children: ReactNode }) {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const [state, setState] = useState<GateState>({ kind: "loading" });
  const [consented, setConsented] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (!token) {
      return;
    }

    let cancelled = false;

    async function loadStatus() {
      try {
        const status = await fetchLegalStatus(token);
        if (cancelled) {
          return;
        }
        if (status.needs_re_acceptance) {
          setState({ kind: "blocked", error: "" });
        } else {
          setState({ kind: "ready" });
        }
      } catch (error) {
        if (cancelled) {
          return;
        }
        setState({
          kind: "blocked",
          error: describeAuthError(
            formatUiMessage(locale, "authErrorServiceUnavailable"),
            error,
            locale,
          ),
        });
      }
    }

    void loadStatus();

    return () => {
      cancelled = true;
    };
  }, [locale, token]);

  if (!token || state.kind === "ready") {
    return <>{children}</>;
  }

  if (state.kind === "loading") {
    return (
      <main className="app-auth-shell">
        <section className="app-surface-card" style={{ maxWidth: "28rem", textAlign: "center" }}>
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "gateCheckingSession")}
          </p>
        </section>
      </main>
    );
  }

  async function handleSubmit() {
    if (!token || !consented) {
      setState({
        kind: "blocked",
        error: formatUiMessage(locale, "legalReacceptanceConsentRequired"),
      });
      return;
    }

    setSubmitting(true);
    setState({ kind: "blocked", error: "" });

    try {
      await recordLegalAcceptance(token, "re_acceptance");
      setState({ kind: "ready" });
    } catch (error) {
      setState({
        kind: "blocked",
        error: describeAuthError(
          formatUiMessage(locale, "authRegisterFailed"),
          error,
          locale,
        ),
      });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="app-auth-shell">
      <section className="app-surface-card" style={{ maxWidth: "32rem", display: "grid", gap: "1rem" }}>
        <div>
          <h1 style={{ margin: 0, fontSize: "1.25rem" }}>
            {formatUiMessage(locale, "legalReacceptanceTitle")}
          </h1>
          <p style={{ margin: "0.5rem 0 0", color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "legalReacceptanceBody")}
          </p>
        </div>
        <ConsentCheckbox onConsentChange={setConsented} />
        {state.error ? <p className="app-notice-banner">{state.error}</p> : null}
        <button
          className="app-button-primary app-button-block"
          disabled={submitting}
          onClick={() => void handleSubmit()}
          type="button"
        >
          {submitting
            ? formatUiMessage(locale, "legalReacceptanceSubmitting")
            : formatUiMessage(locale, "legalReacceptanceConfirm")}
        </button>
      </section>
    </main>
  );
}
