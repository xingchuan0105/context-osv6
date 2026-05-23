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
  document_count: number;
  status_summary: Record<string, number>;
  shared: boolean;
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
  answer_blocks: AnswerBlock[];
  agent_id?: string | null;
  agent_name?: string | null;
  agent_icon?: string | null;
  citations: Citation[];
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

type RawWorkspace = {
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

type RawWorkspaceResponse = {
  notebook: RawWorkspace;
};

type RawWorkspaceSession = {
  id: string;
  notebook_id: string;
  title?: string | null;
  agent_type: string;
  summary?: string | null;
  pinned?: boolean;
  created_at: string;
  updated_at: string;
};

type RawWorkspaceSessionListResponse = {
  sessions: RawWorkspaceSession[];
};

type RawWorkspaceChatMessage = {
  id: number;
  session_id: string;
  role: string;
  content: string;
  answer_blocks?: AnswerBlock[];
  agent_id?: string | null;
  agent_name?: string | null;
  agent_icon?: string | null;
  citations?: Citation[];
  created_at: string;
};

type RawWorkspaceChatMessageListResponse = {
  messages: RawWorkspaceChatMessage[];
};

type RawWorkspaceSource = {
  id: string;
  notebook_id: string;
  notebook_name: string;
  title: string;
  file_name: string;
  status: string;
};

type RawWorkspaceSourceListResponse = {
  sources: RawWorkspaceSource[];
};

type RawWorkspaceNote = {
  id: string;
  notebook_id: string;
  title: string;
  content: string;
  preview: string;
  created_at: string;
  updated_at: string;
  promoted_document_id?: string | null;
  promoted_at?: string | null;
};

type RawWorkspaceNoteListResponse = {
  notes: RawWorkspaceNote[];
};

type RawWorkspaceNoteResponse = {
  note: RawWorkspaceNote;
};

type RawPromoteWorkspaceNoteResponse = {
  note: RawWorkspaceNote;
  source_id: string;
};

type RawWorkspaceSourceContentResponse = {
  content: string;
  summary?: string | null;
};

type RawWorkspaceParsedPreviewItem = {
  kind: string;
  text: string;
  page: number;
  cursor: number;
};

type RawWorkspaceParsedPreviewResponse = {
  items: RawWorkspaceParsedPreviewItem[];
  has_more: boolean;
  next_cursor: number;
  summary?: string | null;
};

type RawWorkspaceCitationLookupResponse = {
  doc_name?: string | null;
  content?: string | null;
  doc_id?: string | null;
  chunk_id?: string | null;
  page?: number | null;
  chunk_type?: string | null;
  asset_id?: string | null;
  caption?: string | null;
  image_url?: string | null;
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

function mapWorkspace(workspace: RawWorkspace): Workspace {
  return {
    workspace_id: workspace.id,
    org_id: workspace.org_id,
    owner_id: workspace.owner_id,
    name: workspace.name,
    title: workspace.title,
    description: workspace.description,
    created_at: workspace.created_at,
    updated_at: workspace.updated_at,
    document_count: workspace.document_count ?? 0,
    status_summary: workspace.status_summary ?? {},
    shared: workspace.shared ?? false,
  };
}

function mapWorkspaceSession(session: RawWorkspaceSession): WorkspaceSession {
  return {
    id: session.id,
    workspace_id: session.notebook_id,
    title: session.title ?? null,
    agent_type: session.agent_type,
    summary: session.summary ?? null,
    pinned: session.pinned ?? false,
    created_at: session.created_at,
    updated_at: session.updated_at,
  };
}

function mapWorkspaceChatMessage(message: RawWorkspaceChatMessage): WorkspaceChatMessage {
  return {
    id: message.id,
    session_id: message.session_id,
    role: message.role,
    content: message.content,
    answer_blocks: message.answer_blocks ?? [],
    agent_id: message.agent_id ?? null,
    agent_name: message.agent_name ?? null,
    agent_icon: message.agent_icon ?? null,
    citations: message.citations ?? [],
    created_at: message.created_at,
  };
}

function mapWorkspaceSource(source: RawWorkspaceSource): WorkspaceSource {
  return {
    id: source.id,
    workspace_id: source.notebook_id,
    workspace_name: source.notebook_name,
    title: source.title,
    file_name: source.file_name,
    status: source.status,
  };
}

function mapWorkspaceNote(note: RawWorkspaceNote): WorkspaceNote {
  return {
    id: note.id,
    workspace_id: note.notebook_id,
    title: note.title,
    content: note.content,
    preview: note.preview,
    created_at: note.created_at,
    updated_at: note.updated_at,
    promoted_document_id: note.promoted_document_id ?? null,
    promoted_at: note.promoted_at ?? null,
  };
}

function mapWorkspaceParsedPreviewResponse(
  response: RawWorkspaceParsedPreviewResponse,
): WorkspaceParsedPreviewResponse {
  return {
    items: response.items.map((item) => ({
      kind: item.kind,
      text: item.text,
      page: item.page,
      cursor: item.cursor,
    })),
    has_more: response.has_more,
    next_cursor: response.next_cursor,
    summary: response.summary ?? null,
  };
}

function mapWorkspaceSourceContentResponse(
  response: RawWorkspaceSourceContentResponse,
): WorkspaceSourceContentResponse {
  return {
    content: response.content,
    summary: response.summary ?? null,
  };
}

function mapWorkspaceCitationLookupResponse(
  response: RawWorkspaceCitationLookupResponse,
): WorkspaceCitationLookupResponse {
  return {
    doc_name: response.doc_name ?? null,
    content: response.content ?? null,
    doc_id: response.doc_id ?? null,
    chunk_id: response.chunk_id ?? null,
    page: response.page ?? null,
    chunk_type: response.chunk_type ?? null,
    asset_id: response.asset_id ?? null,
    caption: response.caption ?? null,
    image_url: response.image_url ?? null,
  };
}

export async function getWorkspace(token: string, workspace_id: string): Promise<WorkspaceResponse> {
  const resp = await request<RawWorkspaceResponse>(
    `/api/v1/notebooks/${workspace_id}`,
    { method: "GET" },
    token,
  );

  return {
    workspace: mapWorkspace(resp.notebook),
  };
}

export async function updateWorkspace(
  token: string,
  workspace_id: string,
  requestBody: { name: string; description: string },
): Promise<WorkspaceResponse> {
  const resp = await request<RawWorkspaceResponse>(
    `/api/v1/notebooks/${workspace_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    workspace: mapWorkspace(resp.notebook),
  };
}

export async function listWorkspaceSessions(
  token: string,
  workspace_id: string,
): Promise<WorkspaceSessionListResponse> {
  const resp = await request<RawWorkspaceSessionListResponse>(
    `/api/v1/chat/sessions?notebook_id=${workspace_id}`,
    { method: "GET" },
    token,
  );

  return {
    sessions: resp.sessions.map(mapWorkspaceSession),
  };
}

export async function createWorkspaceSession(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceSessionRequest,
): Promise<WorkspaceSession> {
  const resp = await request<RawWorkspaceSession>(
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

  return mapWorkspaceSession(resp);
}

export async function getWorkspaceSession(
  token: string,
  session_id: string,
): Promise<WorkspaceSession> {
  const resp = await request<RawWorkspaceSession>(
    `/api/v1/chat/sessions/${session_id}`,
    { method: "GET" },
    token,
  );

  return mapWorkspaceSession(resp);
}

export async function listWorkspaceSessionMessages(
  token: string,
  session_id: string,
): Promise<WorkspaceChatMessageListResponse> {
  const resp = await request<RawWorkspaceChatMessageListResponse>(
    `/api/v1/chat/sessions/${session_id}/messages`,
    { method: "GET" },
    token,
  );

  return {
    messages: resp.messages.map(mapWorkspaceChatMessage),
  };
}

export async function updateWorkspaceSession(
  token: string,
  session_id: string,
  requestBody: UpdateWorkspaceSessionRequest,
): Promise<WorkspaceSession> {
  const resp = await request<RawWorkspaceSession>(
    `/api/v1/chat/sessions/${session_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return mapWorkspaceSession(resp);
}

export async function deleteWorkspaceSession(token: string, session_id: string): Promise<void> {
  await request<EmptyResponse>(`/api/v1/chat/sessions/${session_id}`, { method: "DELETE" }, token);
}

export async function listWorkspaceSources(
  token: string,
  workspace_id: string,
): Promise<WorkspaceSourceListResponse> {
  const resp = await request<RawWorkspaceSourceListResponse>(
    `/api/v1/sources?notebook_id=${workspace_id}`,
    { method: "GET" },
    token,
  );

  return {
    sources: resp.sources.map(mapWorkspaceSource),
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
  const response = await request<RawWorkspaceSourceContentResponse>(
    `/api/v1/documents/${document_id}/content`,
    { method: "GET" },
    token,
  );

  return mapWorkspaceSourceContentResponse(response);
}

export async function getWorkspaceSourceParsedPreview(
  token: string,
  document_id: string,
  cursor = 0,
  limit = 120,
): Promise<WorkspaceParsedPreviewResponse> {
  const response = await request<RawWorkspaceParsedPreviewResponse>(
    `/api/v1/documents/${document_id}/parsed-preview?cursor=${cursor}&limit=${limit}`,
    { method: "GET" },
    token,
  );

  return mapWorkspaceParsedPreviewResponse(response);
}

export async function lookupWorkspaceCitation(
  token: string,
  requestBody: WorkspaceCitationLookupRequest,
): Promise<WorkspaceCitationLookupResponse> {
  const response = await request<RawWorkspaceCitationLookupResponse>(
    "/api/v1/chat/citations/lookup",
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return mapWorkspaceCitationLookupResponse(response);
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
  const resp = await request<RawWorkspaceNoteListResponse>(
    `/api/v1/notebooks/${workspace_id}/notes`,
    { method: "GET" },
    token,
  );

  return {
    notes: resp.notes.map(mapWorkspaceNote),
  };
}

export async function createWorkspaceNote(
  token: string,
  workspace_id: string,
  requestBody: CreateWorkspaceNoteRequest,
): Promise<{ note: WorkspaceNote }> {
  const resp = await request<RawWorkspaceNoteResponse>(
    `/api/v1/notebooks/${workspace_id}/notes`,
    {
      method: "POST",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    note: mapWorkspaceNote(resp.note),
  };
}

export async function updateWorkspaceNote(
  token: string,
  workspace_id: string,
  note_id: string,
  requestBody: UpdateWorkspaceNoteRequest,
): Promise<{ note: WorkspaceNote }> {
  const resp = await request<RawWorkspaceNoteResponse>(
    `/api/v1/notebooks/${workspace_id}/notes/${note_id}`,
    {
      method: "PUT",
      body: JSON.stringify(requestBody),
    },
    token,
  );

  return {
    note: mapWorkspaceNote(resp.note),
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
  const resp = await request<RawPromoteWorkspaceNoteResponse>(
    `/api/v1/notebooks/${workspace_id}/notes/${note_id}/promote-to-source`,
    {
      method: "POST",
      body: JSON.stringify({}),
    },
    token,
  );

  return {
    note: mapWorkspaceNote(resp.note),
    source_id: resp.source_id,
  };
}
