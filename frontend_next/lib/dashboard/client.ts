import { ApiError, buildApiUrl } from "../auth/client";

export type DashboardWorkspace = {
  workspace_id: string;
  org_id: string;
  owner_id: string;
  name: string;
  title: string;
  description: string;
  created_at: string;
  updated_at: string;
  document_count: number;
  status_summary: Record<string, number>;
  shared: boolean;
};

export type DashboardWorkspaceListResponse = {
  workspaces: DashboardWorkspace[];
};

export type DashboardWorkspaceResponse = {
  workspace: DashboardWorkspace;
};

export type CreateWorkspaceRequest = {
  name: string;
  description: string;
};

export type UpdateWorkspaceRequest = {
  name: string;
  description: string;
};

type ErrorEnvelope = {
  error: string;
  message: string;
};

type RawNotebook = {
  id: string;
  org_id: string;
  owner_id: string;
  name: string;
  title: string;
  description: string;
  created_at: string;
  updated_at: string;
  document_count?: number;
  status_summary?: Record<string, number>;
  shared?: boolean;
};

type RawNotebookListResponse = {
  notebooks: RawNotebook[];
};

type RawNotebookResponse = {
  notebook: RawNotebook;
};

type EmptyResponse = Record<string, never>;

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

function mapNotebook(notebook: RawNotebook): DashboardWorkspace {
  return {
    workspace_id: notebook.id,
    org_id: notebook.org_id,
    owner_id: notebook.owner_id,
    name: notebook.name,
    title: notebook.title,
    description: notebook.description,
    created_at: notebook.created_at,
    updated_at: notebook.updated_at,
    document_count: notebook.document_count ?? 0,
    status_summary: notebook.status_summary ?? {},
    shared: notebook.shared ?? false,
  };
}

export async function listWorkspaces(token: string): Promise<DashboardWorkspaceListResponse> {
  const resp = await request<RawNotebookListResponse>("/api/v1/notebooks", { method: "GET" }, token);

  return {
    workspaces: resp.notebooks.map(mapNotebook),
  };
}

export async function createWorkspace(
  token: string,
  requestBody: CreateWorkspaceRequest,
): Promise<DashboardWorkspaceResponse> {
  const resp = await request<RawNotebookResponse>(
    "/api/v1/notebooks",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    workspace: mapNotebook(resp.notebook),
  };
}

export async function updateWorkspace(
  token: string,
  workspace_id: string,
  requestBody: UpdateWorkspaceRequest,
): Promise<DashboardWorkspaceResponse> {
  const resp = await request<RawNotebookResponse>(
    `/api/v1/notebooks/${workspace_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    workspace: mapNotebook(resp.notebook),
  };
}

export async function deleteWorkspace(token: string, workspace_id: string): Promise<void> {
  await request<EmptyResponse>(`/api/v1/notebooks/${workspace_id}`, { method: "DELETE" }, token);
}
