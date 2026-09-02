import { workspaceSessionHref } from "../workspace/session-url";

export type ConversationRouteTarget = {
  id: string;
  workspace_id?: string | null;
};

/** Canonical route for either a personal or Workspace-bound Conversation. */
export function conversationHref(session: ConversationRouteTarget): string {
  const workspaceId = session.workspace_id?.trim();
  if (workspaceId) {
    return workspaceSessionHref(workspaceId, session.id);
  }
  return `/chat/${encodeURIComponent(session.id)}`;
}
