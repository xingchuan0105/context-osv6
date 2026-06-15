import { describe, expect, it } from "vitest";

import {
  WorkspaceNoteSyncState,
  isWorkspaceSourceDocscopeEligible,
  isWorkspaceSourceTerminal,
  sortWorkspaceNotes,
  sortWorkspaceSessions,
  sortWorkspaceSources,
} from "../../lib/workspace/model";

describe("workspace model helpers", () => {
  it("sorts sessions, sources, and notes with the same ordering as the workspace runtime", () => {
    expect(
      sortWorkspaceSessions([
        {
          id: "sess-2",
          workspace_id: "ws-1",
          title: null,
          agent_type: "rag",
          pinned: false,
          created_at: "2026-04-17T00:00:00Z",
          updated_at: "2026-04-17T10:00:00Z",
        },
        {
          id: "sess-1",
          workspace_id: "ws-1",
          title: "Pinned",
          agent_type: "rag",
          pinned: true,
          created_at: "2026-04-16T00:00:00Z",
          updated_at: "2026-04-16T10:00:00Z",
        },
        {
          id: "sess-3",
          workspace_id: "ws-1",
          title: "Older",
          agent_type: "rag",
          pinned: false,
          created_at: "2026-04-15T00:00:00Z",
          updated_at: "2026-04-17T10:00:00Z",
        },
      ]).map((item) => item.id),
    ).toEqual(["sess-1", "sess-2", "sess-3"]);

    expect(
      sortWorkspaceSources(
        [
          {
            id: "src-2",
            workspace_id: "ws-1",
            workspace_name: "Workspace 1",
            title: "Beta",
            file_name: "beta.pdf",
            status: "processing",
          },
          {
            id: "src-1",
            workspace_id: "ws-1",
            workspace_name: "Workspace 1",
            title: "Alpha",
            file_name: "alpha.pdf",
            status: "ready",
          },
          {
            id: "src-3",
            workspace_id: "ws-1",
            workspace_name: "Workspace 1",
            title: "Gamma",
            file_name: "alpha.pdf",
            status: "ready",
          },
        ],
        ["src-2"],
      ).map((item) => item.id),
    ).toEqual(["src-2", "src-1", "src-3"]);

    expect(
      sortWorkspaceNotes([
        {
          id: "note-2",
          workspace_id: "ws-1",
          title: "Bravo",
          content: "Body",
          preview: "Body",
          created_at: "2026-04-17T00:00:00Z",
          updated_at: "2026-04-17T10:00:00Z",
          promoted_document_id: null,
          promoted_at: null,
        },
        {
          id: "note-1",
          workspace_id: "ws-1",
          title: "Alpha",
          content: "Body",
          preview: "Body",
          created_at: "2026-04-16T00:00:00Z",
          updated_at: "2026-04-18T10:00:00Z",
          promoted_document_id: null,
          promoted_at: null,
        },
        {
          id: "note-3",
          workspace_id: "ws-1",
          title: "Zulu",
          content: "Body",
          preview: "Body",
          created_at: "2026-04-15T00:00:00Z",
          updated_at: "2026-04-17T10:00:00Z",
          promoted_document_id: null,
          promoted_at: null,
        },
      ]).map((item) => item.id),
    ).toEqual(["note-1", "note-2", "note-3"]);
  });

  it("classifies source statuses and exposes the note sync state enum", () => {
    expect(isWorkspaceSourceTerminal("completed")).toBe(true);
    expect(isWorkspaceSourceTerminal("failed")).toBe(true);
    expect(isWorkspaceSourceTerminal("queued")).toBe(false);

    expect(isWorkspaceSourceDocscopeEligible("ready")).toBe(true);
    expect(isWorkspaceSourceDocscopeEligible("completed")).toBe(true);
    expect(isWorkspaceSourceDocscopeEligible("processing")).toBe(false);

    expect(WorkspaceNoteSyncState).toEqual({
      Idle: "idle",
      Syncing: "syncing",
      Synced: "synced",
      Error: "error",
    });
  });
});
