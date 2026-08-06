"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

import { AppModal } from "../ui/app-modal";
import { PricingCards } from "./PricingCards";
import type { BillingPlan } from "../../lib/billing/api";
import { billingApi } from "../../lib/billing/api";
import { MARKETING_BILLING_PLANS, plansForInterval } from "../../lib/billing/publicPlans";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";

type UpgradeModalProps = {
  open: boolean;
  onClose: () => void;
};

/**
 * Marketing-oriented upgrade dialog: explain membership vs wallet top-up,
 * show tier cards, and send checkout / top-up to formal pages.
 */
export function UpgradeModal({ open, onClose }: UpgradeModalProps) {
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [allPlans, setAllPlans] = useState<BillingPlan[]>(MARKETING_BILLING_PLANS);

  useEffect(() => {
    if (!open) {
      return;
    }
    void billingApi
      .getPlans()
      .then((response) => {
        const remote = response.plans ?? [];
        const ids = new Set(remote.map((p) => p.plan_id));
        const extras = MARKETING_BILLING_PLANS.filter((p) => !ids.has(p.plan_id));
        setAllPlans(extras.length ? [...remote, ...extras] : remote);
      })
      .catch(() => setAllPlans(MARKETING_BILLING_PLANS));
  }, [open]);

  const tierPlans = useMemo(() => plansForInterval(allPlans, "month"), [allPlans]);

  return (
    <AppModal
      open={open}
      size="lg"
      title={formatUiMessage(locale, "upgradeModal.title")}
      closeLabel={formatUiMessage(locale, "appModal.close")}
      fullPageHref="/pricing"
      fullPageLabel={formatUiMessage(locale, "upgradeModal.openFullPage")}
      testId="upgrade-modal"
      onClose={onClose}
    >
      <div style={{ display: "grid", gap: "1.1rem" }}>
        <p className="app-page-subtitle" style={{ margin: 0 }}>
          {formatUiMessage(locale, "upgradeModal.subtitle")}
        </p>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
            gap: "0.75rem",
          }}
          data-testid="upgrade-modal-explain"
        >
          <article
            style={{
              display: "grid",
              gap: "0.4rem",
              padding: "0.85rem 1rem",
              borderRadius: "0.85rem",
              background: "hsl(var(--surface-muted))",
            }}
          >
            <h3 style={{ margin: 0, fontSize: "0.95rem", fontWeight: 650 }}>
              {formatUiMessage(locale, "upgradeModal.memberTitle")}
            </h3>
            <p className="app-page-subtitle" style={{ margin: 0, fontSize: "0.88rem" }}>
              {formatUiMessage(locale, "upgradeModal.memberBody")}
            </p>
          </article>
          <article
            style={{
              display: "grid",
              gap: "0.4rem",
              padding: "0.85rem 1rem",
              borderRadius: "0.85rem",
              background: "hsl(var(--surface-muted))",
            }}
          >
            <h3 style={{ margin: 0, fontSize: "0.95rem", fontWeight: 650 }}>
              {formatUiMessage(locale, "upgradeModal.topupTitle")}
            </h3>
            <p className="app-page-subtitle" style={{ margin: 0, fontSize: "0.88rem" }}>
              {formatUiMessage(locale, "upgradeModal.topupBody")}
            </p>
          </article>
        </div>

        <PricingCards
          compact
          actionMode="details"
          highlightTier="plus"
          locale={locale}
          plans={tierPlans}
          onSelect={() => {
            onClose();
            router.push("/pricing");
          }}
        />

        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            gap: "0.65rem",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "0.75rem 0.85rem",
            borderRadius: "var(--radius-card)",
            border: "1px solid hsl(var(--border-whisper))",
            background: "hsl(var(--surface-muted) / 0.4)",
          }}
          data-testid="upgrade-modal-topup-strip"
        >
          <p className="app-page-subtitle" style={{ margin: 0, flex: "1 1 12rem" }}>
            {formatUiMessage(locale, "upgradeModal.topupStrip")}
          </p>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.5rem" }}>
            <Link
              className="app-button-primary"
              href="/pricing"
              data-testid="upgrade-modal-pricing-cta"
              onClick={onClose}
            >
              {formatUiMessage(locale, "upgradeModal.pricingCta")}
            </Link>
            <Link
              className="app-button-secondary"
              href="/pricing#topup"
              data-testid="upgrade-modal-topup-cta"
              onClick={onClose}
            >
              {formatUiMessage(locale, "upgradeModal.topupCta")}
            </Link>
          </div>
        </div>
      </div>
    </AppModal>
  );
}
