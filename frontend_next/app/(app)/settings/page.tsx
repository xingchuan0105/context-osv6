import { SettingsSurface } from "../../../components/settings/settings-surface";
import { normalizeSettingsTab } from "../../../components/settings/settings-tabs";

type SettingsPageProps = {
  searchParams?: {
    tab?: string | string[];
  };
};

export default function SettingsPage({ searchParams }: SettingsPageProps) {
  return <SettingsSurface activeTab={normalizeSettingsTab(searchParams?.tab)} />;
}
