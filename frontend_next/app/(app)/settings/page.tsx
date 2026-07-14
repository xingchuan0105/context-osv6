"use client";

import { useSearchParams } from "next/navigation";

import { SettingsSurface } from "../../../components/settings/settings-surface";
import { normalizeSettingsTab } from "../../../components/settings/settings-tabs";

/**
 * Client page so desktop static export (output: export) does not require
 * dynamic searchParams at prerender time.
 */
export default function SettingsPage() {
  const searchParams = useSearchParams();
  return <SettingsSurface activeTab={normalizeSettingsTab(searchParams.get("tab") ?? undefined)} />;
}
