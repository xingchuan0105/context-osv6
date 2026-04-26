import { ApiError, buildApiUrl } from "../auth/client";

type ErrorEnvelope = {
  error: string;
  message: string;
};

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

async function decodeError(response: Response) {
  const raw = await response.text();

  if (!raw.trim()) {
    return new ApiError(response.status, null, `Request failed with status ${response.status}`);
  }

  try {
    const parsed = JSON.parse(raw) as ErrorEnvelope;
    return new ApiError(response.status, parsed.error ?? null, parsed.message ?? raw);
  } catch {
    return new ApiError(response.status, null, raw);
  }
}

async function request<T>(path: string, init: RequestInit = {}, token?: string) {
  const headers = new Headers(init.headers);

  if (!headers.has("Accept")) {
    headers.set("Accept", "application/json");
  }

  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  const response = await fetch(buildApiUrl(path), {
    ...init,
    cache: "no-store",
    headers,
  });

  if (!response.ok) {
    throw await decodeError(response);
  }

  return (await response.json()) as T;
}

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
