"use client";

import Link from "next/link";

import { AppPageFrame } from "../page-frame";
import { ProductChromeFooter } from "../product-chrome-footer";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SettingsPanel } from "./settings-panel";
import { SettingsTabBar } from "./settings-tab-bar";
import type { SettingsTab } from "./settings-tabs";

export function SettingsSurface({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();

  return (
    <AppPageFrame
      title={formatUiMessage(locale, "settings.pageTitle")}
      subtitle={formatUiMessage(locale, "settings.pageSubtitle")}
    >
      <div className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
        <div style={{ display: "flex", justifyContent: "flex-start" }}>
          <Link className="app-link app-link-muted" href="/dashboard" data-testid="settings-back-dashboard">
            {formatUiMessage(locale, "dashboardBackToWorkspaces")}
          </Link>
        </div>
        <SettingsTabBar activeTab={activeTab} />
        <SettingsPanel activeTab={activeTab} />
        <ProductChromeFooter />
      </div>
    </AppPageFrame>
  );
}

