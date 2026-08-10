"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";
import styles from "./share-tabs.module.css";

export type ShareTabId = "chat" | "sources" | "shares";

export type ShareTabBarProps = {
  activeTab: ShareTabId;
  sourceCount: number;
  /** Hide the more-shares tab when the owner has no public profile. */
  showShares: boolean;
  onChange: (tab: ShareTabId) => void;
};

/** X-style underline tab bar under the owner hero. */
export function ShareTabBar({ activeTab, sourceCount, showShares, onChange }: ShareTabBarProps) {
  const { locale } = useUiPreferences();

  const tabs: { id: ShareTabId; label: string; count?: number }[] = [
    { id: "chat", label: formatUiMessage(locale, "sharedPublic.tabChat") },
    {
      id: "sources",
      label: formatUiMessage(locale, "sharedPublic.tabSources"),
      count: sourceCount,
    },
    ...(showShares
      ? [{ id: "shares" as const, label: formatUiMessage(locale, "sharedPublic.tabMoreShares") }]
      : []),
  ];

  return (
    <nav className={styles.tabBar} aria-label={formatUiMessage(locale, "sharedPublic.tabBarLabel")}>
      {tabs.map((tab) => {
        const active = tab.id === activeTab;
        return (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active}
            className={`${styles.tab} ${active ? styles.tabActive : ""}`}
            data-testid={`share-tab-${tab.id}`}
            onClick={() => onChange(tab.id)}
          >
            <span className={styles.tabLabel}>
              {tab.label}
              {typeof tab.count === "number" ? (
                <span className={styles.tabCount}>{tab.count}</span>
              ) : null}
            </span>
          </button>
        );
      })}
    </nav>
  );
}
