"use client";

import Link from "next/link";

import { formatUiMessage } from "../../lib/i18n/messages";
import { appNavHref } from "../../lib/navigation/nav-config";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SettingsPanel } from "./settings-panel";
import { SettingsTabBar } from "./settings-tab-bar";
import type { SettingsTab } from "./settings-tabs";
import styles from "./settings-surface.module.css";

/**
 * Full-viewport settings: left nav + content fills the page (not a centered modal card).
 * Back-to-previous entry sits on the left of the header (uniform across deep pages).
 */
export function SettingsSurface({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();

  return (
    <main className={styles.page} data-testid="settings-surface">
      <div className={styles.shell}>
        <header className={styles.shellHeader}>
          <div className={styles.shellHeaderStart}>
            <Link
              className="app-link app-link-muted"
              data-testid="settings-back-dashboard"
              href={appNavHref("dashboard")}
            >
              {formatUiMessage(locale, "dashboardBackToWorkspaces")}
            </Link>
            <h1 className={styles.shellTitle}>
              {formatUiMessage(locale, "settings.pageTitle")}
            </h1>
          </div>
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
