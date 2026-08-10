import type { UiChatMessage } from "../../hooks/chat-session/types";
import type { WorkspaceSession } from "../workspace/model";

const STORAGE_PREFIX = "context-os.share-sessions.v1:";

export type LocalShareSession = {
  id: string;
  title: string | null;
  pinned: boolean;
  created_at: string;
  updated_at: string;
  messages: UiChatMessage[];
};

function storageKey(shareToken: string) {
  return `${STORAGE_PREFIX}${shareToken.trim()}`;
}

function nowIso() {
  return new Date().toISOString();
}

export function loadLocalShareSessions(shareToken: string): LocalShareSession[] {
  if (typeof window === "undefined" || !shareToken.trim()) {
    return [];
  }
  try {
    const raw = window.localStorage.getItem(storageKey(shareToken));
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .filter((item): item is LocalShareSession => {
        return (
          item != null &&
          typeof item === "object" &&
          typeof (item as LocalShareSession).id === "string"
        );
      })
      .map((item) => ({
        id: item.id,
        title: item.title ?? null,
        pinned: Boolean(item.pinned),
        created_at: item.created_at || nowIso(),
        updated_at: item.updated_at || nowIso(),
        messages: Array.isArray(item.messages) ? item.messages : [],
      }));
  } catch {
    return [];
  }
}

export function saveLocalShareSessions(shareToken: string, sessions: LocalShareSession[]) {
  if (typeof window === "undefined" || !shareToken.trim()) {
    return;
  }
  try {
    window.localStorage.setItem(storageKey(shareToken), JSON.stringify(sessions));
  } catch {
    // ignore quota / private mode
  }
}

export function toWorkspaceSessions(
  sessions: LocalShareSession[],
  workspaceId: string,
): WorkspaceSession[] {
  return sessions.map((session) => ({
    id: session.id,
    workspace_id: workspaceId,
    title: session.title,
    agent_type: "rag",
    pinned: session.pinned,
    created_at: session.created_at,
    updated_at: session.updated_at,
  }));
}

export function createLocalShareSession(): LocalShareSession {
  const ts = nowIso();
  return {
    id: crypto.randomUUID(),
    title: null,
    pinned: false,
    created_at: ts,
    updated_at: ts,
    messages: [],
  };
}

export function deriveSessionTitle(messages: UiChatMessage[]): string | null {
  const firstUser = messages.find((m) => m.role === "user" && m.content.trim());
  if (!firstUser) {
    return null;
  }
  const text = firstUser.content.trim().replace(/\s+/g, " ");
  return text.length > 48 ? `${text.slice(0, 48)}…` : text;
}
