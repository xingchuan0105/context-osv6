import { fetchResponse, request } from "../http/request";
import type {
  WorkspaceNote,
  WorkspaceSession,
  WorkspaceSource,
} from "./model";
import type { AnswerBlock, Citation, ToolResult } from "./stream";

export type Workspace = {
  workspace_id: string;
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

export type RawNotebook = {
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

/** Single notebook DTO → workspace mapping (storage still uses notebook id). */
export function mapNotebook(raw: RawNotebook): Workspace {
  const { id, ...rest } = raw;
  return { ...rest, workspace_id: id };
}

function workspaceFromEnvelope(resp: { workspace?: RawNotebook; notebook?: RawNotebook }): Workspace {
  const raw = resp.workspace ?? resp.notebook;
  if (!raw) {
    throw new Error("workspace envelope missing workspace/notebook");
  }
  return mapNotebook(raw);
}

function workspacesFromListEnvelope(resp: {
  workspaces?: RawNotebook[];
  notebooks?: RawNotebook[];
}): Workspace[] {
  const list = resp.workspaces ?? resp.notebooks ?? [];
  return list.map(mapNotebook);
}

export type WorkspaceResponse = {
  workspace: Workspace;
};

export type WorkspaceSessionListResponse = {
  sessions: WorkspaceSession[];
};

export type WorkspaceChatMessage = {
  id: number;
  session_id: string;
  role: string;
  content: string;
  answer_blocks?: AnswerBlock[];
  agent_id?: string | null;
  agent_name?: string | null;
  agent_icon?: string | null;
  citations?: Citation[];
  tool_results?: ToolResult[] | null;
  created_at: string;
};

export type WorkspaceChatMessageListResponse = {
  messages: WorkspaceChatMessage[];
};

export type CreateWorkspaceSessionRequest = {
  title?: string | null;
  agent_type?: string;
};

export type UpdateWorkspaceSessionRequest = {
  title?: string | null;
  pinned?: boolean | null;
};

export type WorkspaceSourceListResponse = {
  sources: WorkspaceSource[];
};

export type WorkspaceDocumentUploadResponse = {
  document_id: string;
  upload_url: string;
  status: string;
};

export type CreateWorkspaceDocumentUploadRequest = {
  filename: string;
  file_size: number;
  mime_type: string;
};

export type WorkspaceNoteListResponse = {
  notes: WorkspaceNote[];
};

export type CreateWorkspaceNoteRequest = {
  title?: string | null;
  content?: string | null;
};

export type UpdateWorkspaceNoteRequest = {
  title?: string | null;
  content?: string | null;
};

export type PromoteWorkspaceNoteResponse = {
  note: WorkspaceNote;
  source_id: string;
};

export type WorkspaceSourceContentResponse = {
  content: string;
  summary: string | null;
};

export type WorkspaceParsedPreviewItem = {
  kind: string;
  text: string;
  page: number;
  cursor: number;
};

export type WorkspaceParsedPreviewResponse = {
  items: WorkspaceParsedPreviewItem[];
  has_more: boolean;
  next_cursor: number;
  summary: string | null;
};

export type WorkspaceCitationLookupRequest = {
  session_id: string;
  message_id: number;
  citation_id: number;
};

export type WorkspaceCitationLookupResponse = {
  doc_name: string | null;
  content: string | null;
  doc_id: string | null;
  chunk_id: string | null;
  page: number | null;
  chunk_type: string | null;
  asset_id: string | null;
  caption: string | null;
  image_url: string | null;
};

export type WorkspaceMessageFeedbackRequest = {
  session_id: string;
  message_id: number;
  rating: "up" | "down";
};

type EmptyResponse = Record<string, never>;

export async function getWorkspace(token: string, workspace_id: string): Promise<WorkspaceResponse> {
  const resp = await request<{ workspace?: RawNotebook; notebook?: RawNotebook }>(
    `/api/v1/workspaces/${workspace_id}`,
    { method: "GET" },
    token,
  );

  return { workspace: workspaceFromEnvelope(resp) };
}

export async function updateWorkspace(
  token: string,
  workspace_id: string,
  requestBody: { name: string; description: string },
): Promise<WorkspaceResponse> {
  const resp = await request<{ workspace?: RawNotebook; notebook?: RawNotebook }>(
    `/api/v1/workspaces/${workspace_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return { workspace: workspaceFromEnvelope(resp) };
}

export async function listWorkspaceSessions(
  token: string,
  workspace_id: string,
): Promise<WorkspaceSessionListResponse> {
  return request<WorkspaceSessionListResponse>(
    `/api/v1/chat/sessions?workspace_id=${workspace_id}`,
    { method: "GET" },
    token,
  );
}

export async function createWorkspaceSession(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceSessionRequest,
): Promise<WorkspaceSession> {
  return request<WorkspaceSession>(
    "/api/v1/chat/sessions",
    {
      method: "POST",
      body: JSON.stringify({
        workspace_id,
        ...requestBody,
      }),
    },
    token,
  );
}

export async function getWorkspaceSession(
  token: string,
  session_id: string,
): Promise<WorkspaceSession> {
  return request<WorkspaceSession>(
    `/api/v1/chat/sessions/${session_id}`,
    { method: "GET" },
    token,
  );
}

export async function listWorkspaceSessionMessages(
  token: string,
  session_id: string,
): Promise<WorkspaceChatMessageListResponse> {
  const resp = await request<WorkspaceChatMessageListResponse>(
    `/api/v1/chat/sessions/${session_id}/messages`,
    { method: "GET" },
    token,
  );

  return resp;
}

export async function updateWorkspaceSession(
  token: string,
  session_id: string,
  requestBody: UpdateWorkspaceSessionRequest,
): Promise<WorkspaceSession> {
  return request<WorkspaceSession>(
    `/api/v1/chat/sessions/${session_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function deleteWorkspaceSession(token: string, session_id: string): Promise<void> {
  await request<EmptyResponse>(`/api/v1/chat/sessions/${session_id}`, { method: "DELETE" }, token);
}

export async function listWorkspaceSources(
  token: string,
  workspace_id: string,
): Promise<WorkspaceSourceListResponse> {
  return request<WorkspaceSourceListResponse>(
    `/api/v1/sources?workspace_id=${workspace_id}`,
    { method: "GET" },
    token,
  );
}

export async function addWorkspaceSourceUrl(
  token: string,
  workspace_id: string,
  url: string,
): Promise<WorkspaceDocumentUploadResponse> {
  const resp = await request<WorkspaceDocumentUploadResponse>(
    `/api/v1/workspaces/${workspace_id}/sources/url`,
    {
      method: "POST",
      body: JSON.stringify({ url }),
    },
    token,
  );

  return resp;
}

export async function createWorkspaceDocumentUpload(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceDocumentUploadRequest,
): Promise<WorkspaceDocumentUploadResponse> {
  const resp = await request<WorkspaceDocumentUploadResponse>(
    `/api/v1/workspaces/${workspace_id}/documents`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return resp;
}

export async function uploadWorkspaceDocumentFile(
  upload_url: string,
  file: Blob,
): Promise<void> {
  const headers = new Headers();
  headers.set("Content-Type", file.type || "application/octet-stream");

  await fetchResponse(upload_url, {
    method: "PUT",
    headers,
    body: file,
  });
}

export async function completeWorkspaceDocumentUpload(
  token: string,
  document_id: string,
): Promise<void> {
  await request<EmptyResponse>(
    `/api/v1/documents/${document_id}/complete-upload`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function deleteWorkspaceDocument(
  token: string,
  document_id: string,
): Promise<void> {
  await request<EmptyResponse>(`/api/v1/documents/${document_id}`, { method: "DELETE" }, token);
}

export async function reindexWorkspaceDocument(
  token: string,
  document_id: string,
): Promise<void> {
  await request<EmptyResponse>(
    `/api/v1/documents/${document_id}/reindex`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}

export async function getWorkspaceSourceContent(
  token: string,
  document_id: string,
): Promise<WorkspaceSourceContentResponse> {
  const response = await request<WorkspaceSourceContentResponse>(
    `/api/v1/documents/${document_id}/content`,
    { method: "GET" },
    token,
  );

  return response;
}

export async function getWorkspaceSourceParsedPreview(
  token: string,
  document_id: string,
  cursor = 0,
  limit = 120,
): Promise<WorkspaceParsedPreviewResponse> {
  const response = await request<WorkspaceParsedPreviewResponse>(
    `/api/v1/documents/${document_id}/parsed-preview?cursor=${cursor}&limit=${limit}`,
    { method: "GET" },
    token,
  );

  return response;
}

export async function lookupWorkspaceCitation(
  token: string,
  requestBody: WorkspaceCitationLookupRequest,
): Promise<WorkspaceCitationLookupResponse> {
  const response = await request<WorkspaceCitationLookupResponse>(
    "/api/v1/chat/citations/lookup",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return response;
}

export async function submitWorkspaceMessageFeedback(
  token: string,
  requestBody: WorkspaceMessageFeedbackRequest,
): Promise<void> {
  await request<EmptyResponse>(
    `/api/v1/chat/sessions/${requestBody.session_id}/messages/${requestBody.message_id}/feedback`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function listWorkspaceNotes(
  token: string,
  workspace_id: string,
): Promise<WorkspaceNoteListResponse> {
  return request<WorkspaceNoteListResponse>(
    `/api/v1/workspaces/${workspace_id}/notes`,
    { method: "GET" },
    token,
  );
}

export async function createWorkspaceNote(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceNoteRequest,
): Promise<{ note: WorkspaceNote }> {
  return request<{ note: WorkspaceNote }>(
    `/api/v1/workspaces/${workspace_id}/notes`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function updateWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
  requestBody: UpdateWorkspaceNoteRequest,
): Promise<{ note: WorkspaceNote }> {
  return request<{ note: WorkspaceNote }>(
    `/api/v1/workspaces/${workspace_id}/notes/${note_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );
}

export async function deleteWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
): Promise<void> {
  await request<EmptyResponse>(
    `/api/v1/workspaces/${workspace_id}/notes/${note_id}`,
    { method: "DELETE" },
    token,
  );
}

export async function promoteWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
): Promise<PromoteWorkspaceNoteResponse> {
  return request<PromoteWorkspaceNoteResponse>(
    `/api/v1/workspaces/${workspace_id}/notes/${note_id}/promote-to-source`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );
}
