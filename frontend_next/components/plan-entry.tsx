"use client";

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { UpgradeModal } from "./billing/upgrade-modal";
import { SettingsQuickModal } from "./settings/settings-quick-modal";
import { useAuth } from "../lib/auth/context";
import { formatUiMessage } from "../lib/i18n/messages";
import type { UiLocale } from "../lib/i18n/config";
import { getSubscription } from "../lib/settings/client";

const PLAN_ENTRY_QUERY_KEY = "plan-entry-subscription";

function planLabel(planId: string): string {
  const known: Record<string, string> = {
    plus: "Plus",
    pro: "Pro",
  };
  return known[planId] ?? planId;
}

/**
 * First-class subscription entry for product top bars.
 * Free → accent "Upgrade" CTA opens in-page upgrade modal; paid → plan badge opens billing panel modal.
 * Silent while loading or on request failure (no flash, no error surface).
 */
export function PlanEntry({
  locale,
  size = "default",
}: {
  locale: UiLocale;
  size?: "default" | "compact";
}) {
  const { token } = useAuth();
  const [upgradeOpen, setUpgradeOpen] = useState(false);
  const [billingOpen, setBillingOpen] = useState(false);

  const subscriptionQuery = useQuery({
    queryKey: [PLAN_ENTRY_QUERY_KEY, token],
    enabled: Boolean(token),
    staleTime: 60_000,
    queryFn: async () => {
      try {
        return await getSubscription(token as string);
      } catch {
        return null;
      }
    },
  });

  const planId = subscriptionQuery.data?.plan_id?.trim().toLowerCase();

  if (!token || !planId) {
    return null;
  }

  if (planId === "free") {
    return (
      <>
        <button
          className={`app-button-primary app-button-accent plan-entry plan-entry-${size}`}
          data-testid="plan-entry-upgrade"
          type="button"
          onClick={() => setUpgradeOpen(true)}
        >
          {formatUiMessage(locale, "planEntry.upgrade")}
        </button>
        {upgradeOpen ? (
          <UpgradeModal open onClose={() => setUpgradeOpen(false)} />
        ) : null}
      </>
    );
  }

  return (
    <>
      <button
        aria-label={formatUiMessage(locale, "planEntry.viewSubscription")}
        className={`plan-entry-badge plan-entry-${size}`}
        data-testid="plan-entry-badge"
        type="button"
        onClick={() => setBillingOpen(true)}
      >
        {planLabel(planId)}
      </button>
      {billingOpen ? (
        <SettingsQuickModal
          locale={locale}
          open
          tab="billing"
          onClose={() => setBillingOpen(false)}
        />
      ) : null}
    </>
  );
}
