import { request } from "../http/request";
import type { Workspace as ApiWorkspace } from "../contracts/generated";
import { mapWorkspace, type Workspace } from "../workspace/client";

/** Dashboard uses the same UI Workspace shape as workspace client. */
export type DashboardWorkspace = Workspace;

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

type EmptyResponse = Record<string, never>;

function toDashboardWorkspace(raw: ApiWorkspace): DashboardWorkspace {
  const mapped = mapWorkspace(raw);
  return {
    ...mapped,
    document_count: mapped.document_count ?? 0,
    status_summary: mapped.status_summary ?? {},
    shared: mapped.shared ?? false,
  };
}

export async function listWorkspaces(token: string): Promise<DashboardWorkspaceListResponse> {
  const resp = await request<{ workspaces?: ApiWorkspace[] }>("/api/v1/workspaces", { method: "GET" }, token);

  return {
    workspaces: (resp.workspaces ?? []).map(toDashboardWorkspace),
  };
}

export async function createWorkspace(
  token: string,
  requestBody: CreateWorkspaceRequest,
): Promise<DashboardWorkspaceResponse> {
  const resp = await request<{ workspace?: ApiWorkspace }>(
    "/api/v1/workspaces",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  if (!resp.workspace) {
    throw new Error("workspace envelope missing workspace");
  }

  return {
    workspace: toDashboardWorkspace(resp.workspace),
  };
}

export async function updateWorkspace(
  token: string,
  workspace_id: string,
  requestBody: UpdateWorkspaceRequest,
): Promise<DashboardWorkspaceResponse> {
  const resp = await request<{ workspace?: ApiWorkspace }>(
    `/api/v1/workspaces/${workspace_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  if (!resp.workspace) {
    throw new Error("workspace envelope missing workspace");
  }

  return {
    workspace: toDashboardWorkspace(resp.workspace),
  };
}

export async function deleteWorkspace(token: string, workspace_id: string): Promise<void> {
  await request<EmptyResponse>(`/api/v1/workspaces/${workspace_id}`, { method: "DELETE" }, token);
}
