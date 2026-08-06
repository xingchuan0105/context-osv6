"use client";

import { useCallback, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { getReferralStats } from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import { AppModal } from "../ui/app-modal";
import styles from "./referral-invite-surface.module.css";

/**
 * Floating entry + modal for ADR-0010 referral codes.
 * Shown on Dashboard and Workspace (not settings-only).
 */
export function ReferralInviteSurface() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState<"code" | "link" | null>(null);

  const referralQuery = useQuery({
    queryKey: ["referral-fab", auth.token],
    enabled: Boolean(auth.token) && open,
    staleTime: 60_000,
    queryFn: () => getReferralStats(auth.token as string),
  });

  const shareLink = useMemo(() => {
    const code = referralQuery.data?.code?.trim();
    if (!code || typeof window === "undefined") {
      return "";
    }
    const origin = window.location.origin;
    return `${origin}/register?ref=${encodeURIComponent(code)}`;
  }, [referralQuery.data?.code]);

  const copyText = useCallback(async (kind: "code" | "link", value: string) => {
    if (!value) {
      return;
    }
    try {
      await navigator.clipboard.writeText(value);
      setCopied(kind);
      window.setTimeout(() => setCopied(null), 1800);
    } catch {
      /* ignore */
    }
  }, []);

  if (!auth.token) {
    return null;
  }

  const stats = referralQuery.data;
  const remaining = stats?.remaining ?? null;
  const quota = stats?.quota ?? null;
  const rewarded = stats?.rewarded_count ?? null;

  return (
    <>
      <button
        type="button"
        className={styles.fab}
        data-testid="referral-fab"
        aria-label={formatUiMessage(locale, "referral.fabLabel")}
        onClick={() => setOpen(true)}
      >
        <span className={styles.fabIcon} aria-hidden="true">
          ✦
        </span>
        <span className={styles.fabText}>
          {formatUiMessage(locale, "referral.fabText")}
        </span>
        <span className={styles.fabBadge}>¥5</span>
      </button>

      <AppModal
        open={open}
        size="md"
        title={formatUiMessage(locale, "referral.modalTitle")}
        closeLabel={formatUiMessage(locale, "appModal.close")}
        testId="referral-modal"
        onClose={() => setOpen(false)}
      >
        <div className={styles.modalBody}>
          <p className={styles.hero}>
            {formatUiMessage(locale, "referral.hero")}
          </p>
          <ul className={styles.bullets}>
            <li>{formatUiMessage(locale, "referral.bulletBoth")}</li>
            <li>{formatUiMessage(locale, "referral.bulletStack")}</li>
            <li>{formatUiMessage(locale, "referral.bulletQuota")}</li>
            <li>{formatUiMessage(locale, "referral.bulletWalletOnly")}</li>
          </ul>

          {referralQuery.isLoading ? (
            <p className={styles.muted}>{formatUiMessage(locale, "referral.loading")}</p>
          ) : referralQuery.isError ? (
            <p className="app-notice-banner" role="alert">
              {formatUiMessage(locale, "referral.loadError")}
            </p>
          ) : stats ? (
            <div className={styles.card} data-testid="referral-modal-stats">
              <div className={styles.row}>
                <span className={styles.label}>
                  {formatUiMessage(locale, "settings.billing.referralCodeLabel")}
                </span>
                <strong className={styles.code} data-testid="referral-modal-code">
                  {stats.code}
                </strong>
                <button
                  type="button"
                  className="app-button-secondary"
                  onClick={() => void copyText("code", stats.code)}
                >
                  {copied === "code"
                    ? formatUiMessage(locale, "settings.billing.referralCopied")
                    : formatUiMessage(locale, "settings.billing.referralCopy")}
                </button>
              </div>
              {shareLink ? (
                <div className={styles.row}>
                  <span className={styles.label}>
                    {formatUiMessage(locale, "referral.shareLinkLabel")}
                  </span>
                  <code className={styles.linkPreview} title={shareLink}>
                    {shareLink}
                  </code>
                  <button
                    type="button"
                    className="app-button-primary"
                    onClick={() => void copyText("link", shareLink)}
                  >
                    {copied === "link"
                      ? formatUiMessage(locale, "settings.billing.referralCopied")
                      : formatUiMessage(locale, "referral.copyLink")}
                  </button>
                </div>
              ) : null}
              <div className={styles.statsLine}>
                {formatUiMessage(locale, "referral.progress", {
                  rewarded: String(rewarded ?? 0),
                  quota: String(quota ?? "—"),
                  remaining: String(remaining ?? "—"),
                })}
              </div>
              <p className={styles.finePrint}>
                {formatUiMessage(locale, "referral.finePrint")}
              </p>
            </div>
          ) : null}
        </div>
      </AppModal>
    </>
  );
}
