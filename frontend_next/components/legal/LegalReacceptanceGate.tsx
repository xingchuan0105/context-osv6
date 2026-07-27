"use client";

import { type ReactNode, useEffect, useState } from "react";

import ConsentCheckbox from "@/components/legal/ConsentCheckbox";
import { describeAuthError } from "@/lib/auth/errors";
import { useAuth } from "@/lib/auth/context";
import { fetchLegalStatus, recordLegalAcceptance } from "@/lib/legal/client";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";
import styles from "./legal-reacceptance-gate.module.css";

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
    const activeToken = token;
    if (!activeToken) {
      return;
    }

    let cancelled = false;

    async function loadStatus(authToken: string) {
      try {
        const status = await fetchLegalStatus(authToken);
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

    void loadStatus(activeToken);

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
        <section className={`app-surface-card ${styles.loadingCard}`}>
          <p className={styles.loadingText}>
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
      <section className={`app-surface-card ${styles.gateCard}`}>
        <div>
          <h1 className={styles.title}>
            {formatUiMessage(locale, "legalReacceptanceTitle")}
          </h1>
          <p className={styles.body}>
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
