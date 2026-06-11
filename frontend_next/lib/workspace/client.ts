import { ApiError, buildApiUrl } from "../auth/client";
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

type ErrorEnvelope = {
  error: string;
  message: string;
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

export async function getWorkspace(token: string, workspace_id: string): Promise<WorkspaceResponse> {
  const resp = await request<{ notebook: Omit<Workspace, "workspace_id"> & { id: string } }>(
    `/api/v1/notebooks/${workspace_id}`,
    { method: "GET" },
    token,
  );

  const { id, ...notebook } = resp.notebook;
  return {
    workspace: { ...notebook, workspace_id: id },
  };
}

export async function updateWorkspace(
  token: string,
  workspace_id: string,
  requestBody: { name: string; description: string },
): Promise<WorkspaceResponse> {
  const resp = await request<{ notebook: Omit<Workspace, "workspace_id"> & { id: string } }>(
    `/api/v1/notebooks/${workspace_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  const { id, ...notebook } = resp.notebook;
  return {
    workspace: { ...notebook, workspace_id: id },
  };
}

export async function listWorkspaceSessions(
  token: string,
  workspace_id: string,
): Promise<WorkspaceSessionListResponse> {
  const resp = await request<{
    sessions: Array<
      Omit<WorkspaceSession, "workspace_id"> & { notebook_id: string }
    >;
  }>(`/api/v1/chat/sessions?notebook_id=${workspace_id}`, { method: "GET" }, token);

  return {
    sessions: resp.sessions.map(({ notebook_id, ...session }) => ({
      ...session,
      workspace_id: notebook_id,
    })),
  };
}

export async function createWorkspaceSession(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceSessionRequest,
): Promise<WorkspaceSession> {
  const resp = await request<
    Omit<WorkspaceSession, "workspace_id"> & { notebook_id: string }
  >(
    "/api/v1/chat/sessions",
    {
      method: "POST",
      body: JSON.stringify({
        notebook_id: workspace_id,
        ...requestBody,
      }),
    },
    token,
  );

  const { notebook_id, ...session } = resp;
  return {
    ...session,
    workspace_id: notebook_id,
  };
}

export async function getWorkspaceSession(
  token: string,
  session_id: string,
): Promise<WorkspaceSession> {
  const resp = await request<
    Omit<WorkspaceSession, "workspace_id"> & { notebook_id: string }
  >(
    `/api/v1/chat/sessions/${session_id}`,
    { method: "GET" },
    token,
  );

  const { notebook_id, ...session } = resp;
  return {
    ...session,
    workspace_id: notebook_id,
  };
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
  const resp = await request<
    Omit<WorkspaceSession, "workspace_id"> & { notebook_id: string }
  >(
    `/api/v1/chat/sessions/${session_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  const { notebook_id, ...session } = resp;
  return {
    ...session,
    workspace_id: notebook_id,
  };
}

export async function deleteWorkspaceSession(token: string, session_id: string): Promise<void> {
  await request<EmptyResponse>(`/api/v1/chat/sessions/${session_id}`, { method: "DELETE" }, token);
}

export async function listWorkspaceSources(
  token: string,
  workspace_id: string,
): Promise<WorkspaceSourceListResponse> {
  const resp = await request<{
    sources: Array<
      Omit<WorkspaceSource, "workspace_id" | "workspace_name"> & {
        notebook_id: string;
        notebook_name: string;
      }
    >;
  }>(`/api/v1/sources?notebook_id=${workspace_id}`, { method: "GET" }, token);

  return {
    sources: resp.sources.map(({ notebook_id, notebook_name, ...source }) => ({
      ...source,
      workspace_id: notebook_id,
      workspace_name: notebook_name,
    })),
  };
}

export async function addWorkspaceSourceUrl(
  token: string,
  workspace_id: string,
  url: string,
): Promise<WorkspaceDocumentUploadResponse> {
  const resp = await request<WorkspaceDocumentUploadResponse>(
    `/api/v1/notebooks/${workspace_id}/sources/url`,
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
    `/api/v1/notebooks/${workspace_id}/documents`,
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
  const target =
    upload_url.startsWith("http://") || upload_url.startsWith("https://")
      ? upload_url
      : buildApiUrl(upload_url);
  const headers = new Headers();
  headers.set("Content-Type", file.type || "application/octet-stream");

  const response = await fetch(target, {
    method: "PUT",
    cache: "no-store",
    headers,
    body: file,
  });

  if (!response.ok) {
    throw await decodeError(response);
  }
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
  const resp = await request<{
    notes: Array<
      Omit<WorkspaceNote, "workspace_id"> & { notebook_id: string }
    >;
  }>(`/api/v1/notebooks/${workspace_id}/notes`, { method: "GET" }, token);

  return {
    notes: resp.notes.map(({ notebook_id, ...note }) => ({
      ...note,
      workspace_id: notebook_id,
    })),
  };
}

export async function createWorkspaceNote(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceNoteRequest,
): Promise<{ note: WorkspaceNote }> {
  const resp = await request<{
    note: Omit<WorkspaceNote, "workspace_id"> & { notebook_id: string };
  }>(
    `/api/v1/notebooks/${workspace_id}/notes`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  const { notebook_id, ...note } = resp.note;
  return {
    note: { ...note, workspace_id: notebook_id },
  };
}

export async function updateWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
  requestBody: UpdateWorkspaceNoteRequest,
): Promise<{ note: WorkspaceNote }> {
  const resp = await request<{
    note: Omit<WorkspaceNote, "workspace_id"> & { notebook_id: string };
  }>(
    `/api/v1/notebooks/${workspace_id}/notes/${note_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  const { notebook_id, ...note } = resp.note;
  return {
    note: { ...note, workspace_id: notebook_id },
  };
}

export async function deleteWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
): Promise<void> {
  await request<EmptyResponse>(
    `/api/v1/notebooks/${workspace_id}/notes/${note_id}`,
    { method: "DELETE" },
    token,
  );
}

export async function promoteWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
): Promise<PromoteWorkspaceNoteResponse> {
  const resp = await request<{
    note: Omit<WorkspaceNote, "workspace_id"> & { notebook_id: string };
    source_id: string;
  }>(
    `/api/v1/notebooks/${workspace_id}/notes/${note_id}/promote-to-source`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );

  const { notebook_id, ...note } = resp.note;
  return {
    note: { ...note, workspace_id: notebook_id },
    source_id: resp.source_id,
  };
}
