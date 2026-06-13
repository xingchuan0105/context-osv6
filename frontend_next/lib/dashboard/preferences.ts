import { request } from "../http/request";

type DashboardPreferences = {
  favorite_notebook_ids: string[];
  workspace_drafts: Array<Record<string, unknown>>;
  workspace_preferences: Array<Record<string, unknown>>;
  notebook_notes: Array<Record<string, unknown>>;
};

type UserPreferences = {
  dashboard: DashboardPreferences;
  notifications: Record<string, unknown>;
};

export async function getFavoriteWorkspaceIds(token: string): Promise<string[]> {
  const preferences = await request<UserPreferences>("/api/auth/preferences", { method: "GET" }, token);
  return preferences.dashboard.favorite_notebook_ids;
}

export async function updateFavoriteWorkspaceIds(
  token: string,
  favorite_workspace_ids: string[],
): Promise<string[]> {
  const preferences = await request<UserPreferences>("/api/auth/preferences", { method: "GET" }, token);
  preferences.dashboard.favorite_notebook_ids = favorite_workspace_ids;

  const updated = await request<UserPreferences>(
    "/api/auth/preferences",
    {
      method: "PUT",
      body: JSON.stringify(preferences),
    },
    token,
  );

  return updated.dashboard.favorite_notebook_ids;
}
