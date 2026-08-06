"use client";

import Link from "next/link";
import { useState } from "react";

import { UpgradeModal } from "../../billing/upgrade-modal";
import { formatUiMessage } from "../../../lib/i18n/messages";
import type { UiLocale } from "../../../lib/i18n/config";
import type { ShareQuotaSummary } from "../../../lib/share/client";
import styles from "./share-conversion-banner.module.css";

type ShareConversionBannerProps = {
  locale: UiLocale;
  quota: ShareQuotaSummary | null;
  /** Stronger CTA when enable failed due to quota. */
  forced?: boolean;
};

/**
 * Conversion strip on workspace share: sell plan slots + wallet top-up.
 * Share quotas are plan-gated; visitor Q&A on platform keys spends Owner wallet
 * (BYOK still meters for analytics/limits but does not debit wallet).
 */
export function ShareConversionBanner({
  locale,
  quota,
  forced = false,
}: ShareConversionBannerProps) {
  const [upgradeOpen, setUpgradeOpen] = useState(false);

  const used = quota?.used ?? 0;
  const max = quota?.max ?? 0;
  const atCap = max > 0 && used >= max;
  const nearCap = max > 0 && used >= Math.max(1, max - 1);
  const showStrong = forced || atCap;

  if (!quota && !forced) {
    return null;
  }

  return (
    <>
      <section
        className={`${styles.banner} ${showStrong ? styles.bannerStrong : ""}`}
        data-testid="share-conversion-banner"
        data-at-cap={atCap ? "true" : "false"}
      >
        <div className={styles.copy}>
          <strong className={styles.title}>
            {showStrong
              ? formatUiMessage(locale, "shareConversion.titleAtCap")
              : nearCap
                ? formatUiMessage(locale, "shareConversion.titleNearCap")
                : formatUiMessage(locale, "shareConversion.titleDefault")}
          </strong>
          <p className={styles.body}>
            {formatUiMessage(locale, "shareConversion.body", {
              used: String(used),
              max: String(max || "—"),
              plan: quota?.plan_id ?? "free",
            })}
          </p>
          <p className={styles.hint}>{formatUiMessage(locale, "shareConversion.byokHint")}</p>
        </div>
        <div className={styles.actions}>
          <button
            type="button"
            className="app-button-primary app-button-accent"
            data-testid="share-conversion-upgrade"
            onClick={() => setUpgradeOpen(true)}
          >
            {formatUiMessage(locale, "shareConversion.upgradeCta")}
          </button>
          <Link
            className="app-button-secondary"
            data-testid="share-conversion-topup"
            href="/pricing#topup"
          >
            {formatUiMessage(locale, "shareConversion.topupCta")}
          </Link>
          <Link className="app-link" href="/pricing">
            {formatUiMessage(locale, "shareConversion.pricingLink")}
          </Link>
        </div>
      </section>

      {upgradeOpen ? (
        <UpgradeModal open={upgradeOpen} onClose={() => setUpgradeOpen(false)} />
      ) : null}
    </>
  );
}
