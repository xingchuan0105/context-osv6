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
 * Subscription entry rendered as a menu row inside the top-bar share group
 * (PRODUCT_IA §5: 升级折叠进分享组).
 * Free → "Upgrade" opens in-page upgrade modal; paid → plan label opens billing panel modal.
 * Silent while loading or on request failure (no flash, no error surface).
 */
export function PlanEntry({
  locale,
  className,
}: {
  locale: UiLocale;
  /** Menu row class supplied by the containing menu. */
  className?: string;
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

  const isFree = planId === "free";

  return (
    <>
      <button
        aria-label={
          isFree ? undefined : formatUiMessage(locale, "planEntry.viewSubscription")
        }
        className={className}
        data-testid="plan-entry-menuitem"
        role="menuitem"
        type="button"
        onClick={() => (isFree ? setUpgradeOpen(true) : setBillingOpen(true))}
      >
        {isFree ? formatUiMessage(locale, "planEntry.upgrade") : planLabel(planId)}
      </button>
      {upgradeOpen ? (
        <UpgradeModal open onClose={() => setUpgradeOpen(false)} />
      ) : null}
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
