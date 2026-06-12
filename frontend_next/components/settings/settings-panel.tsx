"use client";

import { BillingPanel } from "./settings-billing-panel";
import { ProfilePanel } from "./settings-profile-panel";
import { AppearancePanel } from "./settings-appearance-panel";
import { SecurityPanel } from "./settings-security-panel";
import { NotificationsPanel } from "./settings-notifications-panel";
import type { SettingsTab } from "./settings-tabs";

export function SettingsPanel({ activeTab }: { activeTab: SettingsTab }) {
  switch (activeTab) {
    case "billing":
      return <BillingPanel />;
    case "profile":
      return <ProfilePanel />;
    case "appearance":
      return <AppearancePanel />;
    case "security":
      return <SecurityPanel />;
    case "notifications":
      return <NotificationsPanel />;
  }
}

