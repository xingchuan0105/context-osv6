"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SETTINGS_TABS, type SettingsTab } from "./settings-tabs";

export function SettingsTabBar({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();
  const tabKeyMap: Record<SettingsTab, Parameters<typeof formatUiMessage>[1]> = {
    billing: "settings.tabs.billing",
    profile: "settings.tabs.profile",
    appearance: "settings.tabs.appearance",
    security: "settings.tabs.security",
    notifications: "settings.tabs.notifications",
  };

  return (
    <nav
      aria-label={formatUiMessage(locale, "settings.tabsLabel")}
      className="app-tab-bar"
    >
      {SETTINGS_TABS.map((tab) => (
        <Link
          aria-current={tab === activeTab ? "page" : undefined}
          className={`app-tab-button${tab === activeTab ? " app-tab-button-active" : ""}`}
          href={`/settings?tab=${tab}`}
          key={tab}
        >
          {formatUiMessage(locale, tabKeyMap[tab])}
        </Link>
      ))}
    </nav>
  );
}

