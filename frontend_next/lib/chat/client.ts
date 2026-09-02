import type {
  ChatSession,
  CreateDocumentUploadResponse,
  SessionFileRow,
  SessionFilesResponse,
} from "../contracts/generated";
import { request } from "../http/request";

/** Product-facing summary is the canonical generated Conversation contract. */
export type ConversationSummary = ChatSession;

export type ConversationListResponse = {
  sessions: ConversationSummary[];
};

/** Owner-visible conversations across personal chat and Workspace scopes. */
export async function listChatSessions(token: string): Promise<ConversationListResponse> {
  return request<ConversationListResponse>(
    "/api/v1/chat/sessions",
    { method: "GET" },
    token,
  );
}

/** Exact owner-scoped lookup used to validate a Conversation deep link before rendering it. */
export async function getChatSession(
  token: string,
  sessionId: string,
): Promise<ConversationSummary> {
  return request<ConversationSummary>(
    `/api/v1/chat/sessions/${encodeURIComponent(sessionId)}`,
    { method: "GET" },
    token,
  );
}

/** Global-recent order shared by Chat and Cmd/Ctrl+K. */
export function sortConversationsByRecent(
  sessions: readonly ConversationSummary[],
): ConversationSummary[] {
  return [...sessions].sort((left, right) => {
    if (left.updated_at !== right.updated_at) {
      return left.updated_at > right.updated_at ? -1 : 1;
    }
    if (left.id === right.id) {
      return 0;
    }
    return left.id < right.id ? -1 : 1;
  });
}

/** Idempotent first-upload path: create the personal Conversation explicitly. */
export async function createPersonalChatSession(
  token: string,
): Promise<ConversationSummary> {
  return request<ConversationSummary>(
    "/api/v1/chat/sessions",
    { method: "POST", body: JSON.stringify({}) },
    token,
  );
}

/** Presign + binding creation for a Conversation file (chat-first W2b). */
export async function createChatSessionFileUpload(
  token: string,
  sessionId: string,
  file: { filename: string; file_size: number; mime_type: string },
): Promise<CreateDocumentUploadResponse> {
  return request<CreateDocumentUploadResponse>(
    `/api/v1/chat/sessions/${encodeURIComponent(sessionId)}/files`,
    { method: "POST", body: JSON.stringify(file) },
    token,
  );
}

/** Artifact-level completion shared with workspace uploads (parse enqueue). */
export async function completeChatSessionFileUpload(
  token: string,
  documentId: string,
): Promise<{ status: string }> {
  return request<{ status: string }>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/complete-upload`,
    { method: "POST" },
    token,
  );
}

/** Owner-scoped Conversation files with per-file parse state. */
export async function listChatSessionFiles(
  token: string,
  sessionId: string,
): Promise<SessionFileRow[]> {
  const response = await request<SessionFilesResponse>(
    `/api/v1/chat/sessions/${encodeURIComponent(sessionId)}/files`,
    { method: "GET" },
    token,
  );
  return response.files;
}

/** Remove one Conversation binding; zero-binding cleanup is server-side async. */
export async function deleteChatSessionFile(
  token: string,
  sessionId: string,
  bindingId: string,
): Promise<{ status: string }> {
  return request<{ status: string }>(
    `/api/v1/chat/sessions/${encodeURIComponent(sessionId)}/files/${encodeURIComponent(bindingId)}`,
    { method: "DELETE" },
    token,
  );
}

/** Retry parsing for a failed file (artifact-level reindex reuse). */
export async function reindexChatSessionFile(
  token: string,
  documentId: string,
): Promise<{ status: string }> {
  return request<{ status: string }>(
    `/api/v1/documents/${encodeURIComponent(documentId)}/reindex`,
    { method: "POST" },
    token,
  );
}
