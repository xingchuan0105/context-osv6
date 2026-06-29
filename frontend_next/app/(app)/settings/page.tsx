import { SettingsSurface } from "../../../components/settings/settings-surface";
import { normalizeSettingsTab } from "../../../components/settings/settings-tabs";

type SettingsPageProps = {
  searchParams?: Promise<{
    tab?: string | string[];
  }>;
};

export default async function SettingsPage({ searchParams }: SettingsPageProps) {
  const params = searchParams ? await searchParams : undefined;
  return <SettingsSurface activeTab={normalizeSettingsTab(params?.tab)} />;
}
