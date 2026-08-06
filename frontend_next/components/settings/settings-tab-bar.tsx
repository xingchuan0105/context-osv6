"use client";

import Link from "next/link";
import { useMemo, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SETTINGS_TABS, type SettingsTab } from "./settings-tabs";
import styles from "./settings-surface.module.css";

export function SettingsTabBar({
  activeTab,
  onSelect,
}: {
  activeTab: SettingsTab;
  /** When set, use buttons (modal) instead of route links. */
  onSelect?: (tab: SettingsTab) => void;
}) {
  const { locale } = useUiPreferences();
  const [query, setQuery] = useState("");
  const tabKeyMap: Record<SettingsTab, Parameters<typeof formatUiMessage>[1]> = {
    billing: "settings.tabs.billing",
    profile: "settings.tabs.profile",
    providers: "settings.tabs.providers",
    preferences: "settings.tabs.appearance",
    security: "settings.tabs.security",
  };

  const labels = useMemo(
    () =>
      SETTINGS_TABS.map((tab) => ({
        tab,
        label: formatUiMessage(locale, tabKeyMap[tab]),
      })),
    [locale],
  );

  const filtered = labels.filter((item) => {
    if (!query.trim()) {
      return true;
    }
    return item.label.toLowerCase().includes(query.trim().toLowerCase());
  });

  return (
    <div className={styles.navRail} data-testid="settings-nav-rail">
      <label className={styles.searchLabel} htmlFor="settings-nav-search">
        <span className="dashboard-sr-only">
          {formatUiMessage(locale, "settingsTabBar.searchLabel")}
        </span>
        <input
          className={`app-input ${styles.searchInput}`}
          data-testid="settings-nav-search"
          id="settings-nav-search"
          placeholder={formatUiMessage(locale, "settingsTabBar.searchPlaceholder")}
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
      </label>
      <nav
        aria-label={formatUiMessage(locale, "settings.tabsLabel")}
        className={styles.navList}
      >
        {filtered.map(({ tab, label }) =>
          onSelect ? (
            <button
              aria-current={tab === activeTab ? "page" : undefined}
              className={`${styles.navItem}${tab === activeTab ? ` ${styles.navItemActive}` : ""}`}
              key={tab}
              type="button"
              onClick={() => onSelect(tab)}
            >
              {label}
            </button>
          ) : (
            <Link
              aria-current={tab === activeTab ? "page" : undefined}
              className={`${styles.navItem}${tab === activeTab ? ` ${styles.navItemActive}` : ""}`}
              href={`/settings?tab=${tab}`}
              key={tab}
            >
              {label}
            </Link>
          ),
        )}
      </nav>
    </div>
  );
}
