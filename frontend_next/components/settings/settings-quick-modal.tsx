"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { AppModal } from "../ui/app-modal";
import { UpgradeModal } from "../billing/upgrade-modal";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";
import { AppearancePanel } from "./settings-appearance-panel";
import { BillingPanel } from "./settings-billing-panel";
import { ProfilePanel } from "./settings-profile-panel";
import { SecurityPanel } from "./settings-security-panel";

export type SettingsQuickTab = "profile" | "preferences" | "billing" | "security";

type SettingsQuickModalProps = {
  open: boolean;
  tab: SettingsQuickTab | null;
  locale: UiLocale;
  onClose: () => void;
};

function titleForTab(locale: UiLocale, tab: SettingsQuickTab): string {
  switch (tab) {
    case "profile":
      return formatUiMessage(locale, "dashboardProfileLink");
    case "preferences":
      return formatUiMessage(locale, "dashboardAppearanceLink");
    case "billing":
      return formatUiMessage(locale, "dashboardBillingLink");
    case "security":
      return formatUiMessage(locale, "settingsQuickModal.securityLink");
  }
}

function fullPageHref(tab: SettingsQuickTab): string {
  const map: Record<SettingsQuickTab, string> = {
    profile: "profile",
    preferences: "preferences",
    billing: "billing",
    security: "security",
  };
  return `/settings?tab=${map[tab]}`;
}

export function SettingsQuickModal({
  open,
  tab,
  locale,
  onClose,
}: SettingsQuickModalProps) {
  const [upgradeOpen, setUpgradeOpen] = useState(false);

  useEffect(() => {
    if (!open) {
      setUpgradeOpen(false);
    }
  }, [open]);

  if (!tab || (!open && !upgradeOpen)) {
    return null;
  }

  return (
    <>
      <AppModal
        open={open && !upgradeOpen}
        size={tab === "billing" ? "lg" : "md"}
        title={titleForTab(locale, tab)}
        closeLabel={formatUiMessage(locale, "appModal.close")}
        fullPageHref={fullPageHref(tab)}
        fullPageLabel={formatUiMessage(locale, "settingsQuickModal.openFullPage")}
        testId={`settings-quick-modal-${tab}`}
        onClose={onClose}
        footer={
          tab === "billing" ? (
            <button
              className="app-button-primary app-button-accent"
              data-testid="settings-quick-upgrade"
              type="button"
              onClick={() => setUpgradeOpen(true)}
            >
              {formatUiMessage(locale, "settings.billing.managePlanAction")}
            </button>
          ) : (
            <Link className="app-link app-link-muted" href="/settings">
              {formatUiMessage(locale, "settingsQuickModal.moreSettings")}
            </Link>
          )
        }
      >
        {tab === "profile" ? <ProfilePanel /> : null}
        {tab === "preferences" ? <AppearancePanel /> : null}
        {tab === "billing" ? <BillingPanel hideManagePlan /> : null}
        {tab === "security" ? <SecurityPanel /> : null}
      </AppModal>
      <UpgradeModal open={upgradeOpen} onClose={() => setUpgradeOpen(false)} />
    </>
  );
}
