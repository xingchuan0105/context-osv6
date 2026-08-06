"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { acceptInvite, declineInvite } from "../../lib/share/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { getWorkspace } from "../../lib/workspace/client";
import styles from "./invite-surface.module.css";

type InviteSurfaceProps = {
  memberId: string;
  workspaceId: string;
};

type InviteDecision = "accepted" | "declined" | null;

export function InviteSurface({ memberId, workspaceId }: InviteSurfaceProps) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [loading, setLoading] = useState(true);
  const [actionLoading, setActionLoading] = useState(false);
  const [error, setError] = useState("");
  const [workspaceTitle, setWorkspaceTitle] = useState("");
  const [decision, setDecision] = useState<InviteDecision>(null);

  const nextPath = useMemo(
    () => encodeURIComponent(`/invite/${workspaceId}/${memberId}`),
    [memberId, workspaceId],
  );

  const titleOrFallback =
    workspaceTitle.trim() || formatUiMessage(locale, "sharedPublic.inviteWorkspaceFallback");

  useEffect(() => {
    let cancelled = false;

    async function loadInvite() {
      if (!auth.initialized) {
        return;
      }

      if (!workspaceId.trim() || !memberId.trim()) {
        setError(formatUiMessage(locale, "sharedPublic.inviteInvalidLink"));
        setLoading(false);
        return;
      }

      if (!auth.token) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");

      try {
        const response = await getWorkspace(auth.token, workspaceId);

        if (!cancelled) {
          setWorkspaceTitle(response.workspace.title || response.workspace.name || workspaceId);
        }
      } catch {
        if (!cancelled) {
          setWorkspaceTitle(workspaceId);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadInvite();

    return () => {
      cancelled = true;
    };
  }, [auth.initialized, auth.token, locale, memberId, workspaceId]);

  async function handleAccept() {
    if (!auth.token) {
      router.push(`/login?next=${nextPath}`);
      return;
    }

    setActionLoading(true);
    setError("");

    try {
      await acceptInvite(auth.token, workspaceId, memberId);
      setDecision("accepted");
    } catch (acceptError) {
      setError(
        acceptError instanceof Error
          ? acceptError.message
          : formatUiMessage(locale, "sharedPublic.inviteAcceptFailed"),
      );
    } finally {
      setActionLoading(false);
    }
  }

  async function handleDecline() {
    if (!auth.token) {
      router.push(`/login?next=${nextPath}`);
      return;
    }

    setActionLoading(true);
    setError("");

    try {
      await declineInvite(auth.token, workspaceId, memberId);
      setDecision("declined");
    } catch (declineError) {
      setError(
        declineError instanceof Error
          ? declineError.message
          : formatUiMessage(locale, "sharedPublic.inviteDeclineFailed"),
      );
    } finally {
      setActionLoading(false);
    }
  }

  return (
    <main className="app-auth-shell" data-testid="invite-surface">
      <section className={`app-surface-card ${styles.card}`}>
        {loading ? (
          <p className={styles.flushText}>{formatUiMessage(locale, "sharedPublic.inviteLoading")}</p>
        ) : error ? (
          <>
            <h1 className={`app-page-title ${styles.title}`}>
              {formatUiMessage(locale, "sharedPublic.inviteErrorTitle")}
            </h1>
            <p className="app-notice-banner">{error}</p>
          </>
        ) : decision === "accepted" ? (
          <>
            <h1 className={`app-page-title ${styles.title}`}>
              {formatUiMessage(locale, "sharedPublic.inviteAcceptedTitle")}
            </h1>
            <p className="app-page-subtitle">
              {formatUiMessage(locale, "sharedPublic.inviteAcceptedBody", {
                title: titleOrFallback,
              })}
            </p>
            <div className="app-button-row">
              <Link className="app-button-primary" href={`/dashboard/${workspaceId}`}>
                {formatUiMessage(locale, "sharedPublic.openWorkspaceAction")}
              </Link>
            </div>
          </>
        ) : decision === "declined" ? (
          <>
            <h1 className={`app-page-title ${styles.title}`}>
              {formatUiMessage(locale, "sharedPublic.inviteDeclinedTitle")}
            </h1>
            <p className="app-page-subtitle">
              {formatUiMessage(locale, "sharedPublic.inviteDeclinedBody", {
                title: titleOrFallback,
              })}
            </p>
            <div className="app-button-row">
              <Link className="app-button-secondary" href="/">
                {formatUiMessage(locale, "sharedPublic.backHomeAction")}
              </Link>
            </div>
          </>
        ) : (
          <>
            <h1 className={`app-page-title ${styles.title}`}>
              {formatUiMessage(locale, "sharedPublic.inviteTitle")}
            </h1>
            <p className="app-page-subtitle">
              {workspaceTitle
                ? formatUiMessage(locale, "sharedPublic.inviteNamedBody", {
                    title: workspaceTitle,
                  })
                : formatUiMessage(locale, "sharedPublic.inviteGenericBody")}
            </p>

            {!auth.token ? (
              <div className={`app-inline-surface ${styles.authBox}`}>
                <p className={styles.mutedText}>
                  {formatUiMessage(locale, "sharedPublic.inviteAuthHint")}
                </p>
                <div className="app-button-row">
                  <Link className="app-button-primary" href={`/login?next=${nextPath}`}>
                    {formatUiMessage(locale, "sharedPublic.signInToContinueAction")}
                  </Link>
                  <Link className="app-button-secondary" href={`/register?next=${nextPath}`}>
                    {formatUiMessage(locale, "sharedPublic.signUpToContinueAction")}
                  </Link>
                </div>
              </div>
            ) : (
              <div className="app-button-row">
                <button
                  className="app-button-primary"
                  data-testid="invite-accept-button"
                  disabled={actionLoading}
                  type="button"
                  onClick={() => void handleAccept()}
                >
                  {actionLoading
                    ? formatUiMessage(locale, "sharedPublic.inviteProcessing")
                    : formatUiMessage(locale, "sharedPublic.acceptInviteAction")}
                </button>
                <button
                  className="app-button-secondary"
                  data-testid="invite-decline-button"
                  disabled={actionLoading}
                  type="button"
                  onClick={() => void handleDecline()}
                >
                  {formatUiMessage(locale, "sharedPublic.declineInviteAction")}
                </button>
              </div>
            )}
          </>
        )}
      </section>
    </main>
  );
}
