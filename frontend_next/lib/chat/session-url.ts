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

import { formatUiMessage } from "../i18n/messages";
import { ConversationScopeKind } from "../contracts/generated";

type LabelLocale = "zh-CN" | "en";

type ConversationLabelTarget = {
  title?: string | null;
  scope_kind?: string;
  workspace_name?: string | null;
  workspace_id?: string | null;
};

/** Single source of truth for a Conversation's display title (shared fallback). */
export function conversationTitle(
  locale: LabelLocale,
  session: ConversationLabelTarget,
): string {
  return (
    session.title?.trim() ||
    formatUiMessage(locale, "commandPalette.sessionUntitled")
  );
}

/** Single source of truth for the scope label (workspace name / personal). */
export function conversationScopeLabel(
  locale: LabelLocale,
  session: ConversationLabelTarget,
): string {
  const workspaceName = session.workspace_name?.trim() || session.workspace_id?.trim() || "";
  if (session.scope_kind === ConversationScopeKind.Workspace) {
    return workspaceName
      ? formatUiMessage(locale, "chat.workspaceContext", { name: workspaceName })
      : formatUiMessage(locale, "chat.workspaces");
  }
  return formatUiMessage(locale, "chat.personalContext");
}
