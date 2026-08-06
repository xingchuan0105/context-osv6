import { request } from "../http/request";

/** GET /api/v1/search — global product index (workspaces, sessions, sources). */
export type GlobalSearchWorkspace = {
  id: string;
  title?: string;
  name?: string;
  description?: string;
};

export type GlobalSearchSession = {
  id: string;
  workspace_id: string;
  title?: string | null;
  updated_at?: string;
};

export type GlobalSearchSource = {
  id: string;
  workspace_id: string;
  file_name?: string;
  title?: string;
  workspace_name?: string;
};

export type GlobalSearchResponse = {
  workspaces: GlobalSearchWorkspace[];
  sessions: GlobalSearchSession[];
  sources: GlobalSearchSource[];
};

export async function searchProductIndex(
  token: string,
  query: string,
): Promise<GlobalSearchResponse> {
  const q = query.trim();
  if (!q) {
    return { workspaces: [], sessions: [], sources: [] };
  }
  const path = `/api/v1/search?q=${encodeURIComponent(q)}`;
  const resp = await request<Partial<GlobalSearchResponse>>(path, { method: "GET" }, token);
  return {
    workspaces: resp.workspaces ?? [],
    sessions: resp.sessions ?? [],
    sources: resp.sources ?? [],
  };
}
