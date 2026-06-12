"use client";

import { AppPageFrame } from "../page-frame";
import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { SettingsPanel } from "./settings-panel";
import { SettingsTabBar } from "./settings-tab-bar";
import type { SettingsTab } from "./settings-tabs";

export function SettingsSurface({ activeTab }: { activeTab: SettingsTab }) {
  const { locale } = useUiPreferences();

  return (
    <AppPageFrame
      title={formatSettingsShareMessage(locale, "settings.pageTitle")}
      subtitle={formatSettingsShareMessage(locale, "settings.pageSubtitle")}
    >
      <div className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
        <SettingsTabBar activeTab={activeTab} />
        <SettingsPanel activeTab={activeTab} />
      </div>
    </AppPageFrame>
  );
}

