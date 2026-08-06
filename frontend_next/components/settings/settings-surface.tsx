"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SettingsPanel } from "./settings-panel";
import { SettingsTabBar } from "./settings-tab-bar";
import type { SettingsTab } from "./settings-tabs";
import styles from "./settings-surface.module.css";

/**
 * Full-viewport settings: left nav + content fills the page (not a centered modal card).
 */
export function SettingsSurface({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();

  return (
    <main className={styles.page} data-testid="settings-surface">
      <div className={styles.shell}>
        <header className={styles.shellHeader}>
          <h1 className={styles.shellTitle}>
            {formatUiMessage(locale, "settings.pageTitle")}
          </h1>
          <Link
            className="app-button-ghost"
            data-testid="settings-back-dashboard"
            href="/dashboard"
          >
            {formatUiMessage(locale, "dashboardBackToWorkspaces")}
          </Link>
        </header>
        <div className={styles.shellBody}>
          <SettingsTabBar activeTab={activeTab} />
          <div className={styles.content}>
            <SettingsPanel activeTab={activeTab} />
          </div>
        </div>
      </div>
    </main>
  );
}
