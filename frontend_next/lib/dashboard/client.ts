import { request } from "../http/request";
import { type RawNotebook } from "../workspace/client";

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

type RawNotebookListResponse = {
  workspaces?: RawNotebook[];
  notebooks?: RawNotebook[];
};

type RawNotebookResponse = {
  workspace?: RawNotebook;
  notebook?: RawNotebook;
};

type EmptyResponse = Record<string, never>;

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
  const resp = await request<RawNotebookListResponse>("/api/v1/workspaces", { method: "GET" }, token);

  return {
    workspaces: (resp.workspaces ?? resp.notebooks ?? []).map(mapNotebook),
  };
}

export async function createWorkspace(
  token: string,
  requestBody: CreateWorkspaceRequest,
): Promise<DashboardWorkspaceResponse> {
  const resp = await request<RawNotebookResponse>(
    "/api/v1/workspaces",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    workspace: mapNotebook((resp.workspace ?? resp.notebook)!),
  };
}

export async function updateWorkspace(
  token: string,
  workspace_id: string,
  requestBody: UpdateWorkspaceRequest,
): Promise<DashboardWorkspaceResponse> {
  const resp = await request<RawNotebookResponse>(
    `/api/v1/workspaces/${workspace_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    workspace: mapNotebook((resp.workspace ?? resp.notebook)!),
  };
}

export async function deleteWorkspace(token: string, workspace_id: string): Promise<void> {
  await request<EmptyResponse>(`/api/v1/workspaces/${workspace_id}`, { method: "DELETE" }, token);
}
