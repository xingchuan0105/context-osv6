"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { AppModal } from "../ui/app-modal";
import { NavRail } from "../ui/nav-rail";
import { UpgradeModal } from "../billing/upgrade-modal";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";
import { AppearancePanel } from "./settings-appearance-panel";
import { BillingPanel } from "./settings-billing-panel";
import { ProfilePanel } from "./settings-profile-panel";
import { SecurityPanel } from "./settings-security-panel";
import { settingsTabIcon } from "./settings-nav-icons";
import { settingsTabLabelKey } from "./settings-tabs";

export type SettingsQuickTab = "profile" | "preferences" | "billing" | "security";

const QUICK_TABS: SettingsQuickTab[] = ["profile", "preferences", "billing", "security"];

type SettingsQuickModalProps = {
  open: boolean;
  tab: SettingsQuickTab | null;
  locale: UiLocale;
  onClose: () => void;
};

export function SettingsQuickModal({
  open,
  tab,
  locale,
  onClose,
}: SettingsQuickModalProps) {
  const [upgradeOpen, setUpgradeOpen] = useState(false);
  const [activeTab, setActiveTab] = useState<SettingsQuickTab>(tab ?? "profile");

  useEffect(() => {
    if (!open) {
      setUpgradeOpen(false);
      return;
    }
    if (tab) {
      setActiveTab(tab);
    }
  }, [open, tab]);

  if (!tab || (!open && !upgradeOpen)) {
    return null;
  }

  return (
    <>
      <AppModal
        open={open && !upgradeOpen}
        size="xl"
        bodyVariant="rail"
        title={formatUiMessage(locale, "settings.pageTitle")}
        closeLabel={formatUiMessage(locale, "appModal.close")}
        fullPageHref={`/settings?tab=${activeTab}`}
        fullPageLabel={formatUiMessage(locale, "settingsQuickModal.openFullPage")}
        testId={`settings-quick-modal-${tab}`}
        onClose={onClose}
        footer={
          <Link className="app-link app-link-muted" href="/settings">
            {formatUiMessage(locale, "settingsQuickModal.moreSettings")}
          </Link>
        }
      >
        {/* Grok 式：左导航 + 右设置面板，与 /settings 页同一模式 */}
        <NavRail
          activeId={activeTab}
          ariaLabel={formatUiMessage(locale, "settings.tabsLabel")}
          items={QUICK_TABS.map((quickTab) => ({
            id: quickTab as string,
            label: formatUiMessage(locale, settingsTabLabelKey(quickTab)),
            icon: settingsTabIcon(quickTab),
          }))}
          testId="settings-quick-nav-rail"
          onSelect={(id) => setActiveTab(id as SettingsQuickTab)}
        />
        <div
          style={{
            display: "grid",
            gap: "1rem",
            alignContent: "start",
            minWidth: 0,
            padding: "var(--space-lg)",
          }}
        >
          {activeTab === "profile" ? <ProfilePanel /> : null}
          {activeTab === "preferences" ? <AppearancePanel /> : null}
          {activeTab === "billing" ? (
            <BillingPanel onManagePlan={() => setUpgradeOpen(true)} />
          ) : null}
          {activeTab === "security" ? <SecurityPanel /> : null}
        </div>
      </AppModal>
      <UpgradeModal open={upgradeOpen} onClose={() => setUpgradeOpen(false)} />
    </>
  );
}
