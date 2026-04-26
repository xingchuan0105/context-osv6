import type { Citation } from "./stream";

export type WebSource = {
  title: string;
  url: string;
  snippet: string;
};

export type WorkspaceWebSourcesRequest = {
  sources: WebSource[];
};

export type WorkspaceCitationAnchor = {
  top: number;
  left: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
};

export type WorkspaceSession = {
  id: string;
  workspace_id: string;
  title: string | null;
  agent_type: string;
  summary: string | null;
  pinned: boolean;
  created_at: string;
  updated_at: string;
};

export type WorkspaceSource = {
  id: string;
  workspace_id: string;
  workspace_name: string;
  title: string;
  file_name: string;
  status: string;
};

export type WorkspaceNote = {
  id: string;
  workspace_id: string;
  title: string;
  content: string;
  preview: string;
  created_at: string;
  updated_at: string;
  promoted_document_id: string | null;
  promoted_at: string | null;
};

export type WorkspaceCitationRequest = {
  session_id: string;
  message_id: number;
  citation: Citation;
  anchorRect?: WorkspaceCitationAnchor | null;
};

export enum WorkspaceNoteSyncState {
  Idle = "idle",
  Syncing = "syncing",
  Synced = "synced",
  Error = "error",
}

export function sortWorkspaceSessions(sessions: readonly WorkspaceSession[]) {
  return [...sessions].sort((left, right) => {
    if (left.pinned !== right.pinned) {
      return left.pinned ? -1 : 1;
    }

    return right.updated_at.localeCompare(left.updated_at) || left.id.localeCompare(right.id);
  });
}

export function sortWorkspaceSources(
  sources: readonly WorkspaceSource[],
  pinnedSourceIds: readonly string[] = [],
) {
  return [...sources].sort((left, right) => {
    const leftPinned = pinnedSourceIds.includes(left.id);
    const rightPinned = pinnedSourceIds.includes(right.id);

    if (leftPinned !== rightPinned) {
      return leftPinned ? -1 : 1;
    }

    return (
      left.file_name.toLowerCase().localeCompare(right.file_name.toLowerCase()) ||
      left.id.localeCompare(right.id)
    );
  });
}

export function sortWorkspaceNotes(notes: readonly WorkspaceNote[]) {
  return [...notes].sort((left, right) => {
    return right.updated_at.localeCompare(left.updated_at) || left.title.localeCompare(right.title);
  });
}

export function isWorkspaceSourceTerminal(status: string) {
  return status === "completed" || status === "failed" || status === "ready" || status === "error";
}

export function isWorkspaceSourceDocscopeEligible(status: string) {
  return status === "completed" || status === "ready";
}
