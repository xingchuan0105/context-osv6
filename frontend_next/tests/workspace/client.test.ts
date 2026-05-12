import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  addWorkspaceSourceUrl,
  completeWorkspaceDocumentUpload,
  createWorkspaceDocumentUpload,
  createWorkspaceNote,
  createWorkspaceSession,
  deleteWorkspaceDocument,
  deleteWorkspaceNote,
  getWorkspace,
  getWorkspaceSession,
  listWorkspaceSessionMessages,
  listWorkspaceNotes,
  listWorkspaceSessions,
  listWorkspaceSources,
  promoteWorkspaceNote,
  reindexWorkspaceDocument,
  uploadWorkspaceDocumentFile,
  updateWorkspace,
  updateWorkspaceNote,
  updateWorkspaceSession,
} from "../../lib/workspace/client";

const fetchMock = vi.fn();

beforeEach(() => {
  process.env.NEXT_PUBLIC_API_BASE_URL = "https://api.example.test";
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  delete process.env.NEXT_PUBLIC_API_BASE_URL;
  vi.unstubAllGlobals();
});

describe("workspace client", () => {
  it("reads and updates workspaces through the notebook transport while keeping workspace_id at the boundary", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            notebook: {
              id: "ws-1",
              org_id: "org-1",
              owner_id: "user-1",
              name: "Workspace 1",
              title: "Workspace 1",
              description: "Description",
              created_at: "2026-04-17T00:00:00Z",
              updated_at: "2026-04-18T00:00:00Z",
              document_count: 3,
              status_summary: { ready: 2 },
              shared: true,
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            notebook: {
              id: "ws-1",
              org_id: "org-1",
              owner_id: "user-1",
              name: "Workspace 1",
              title: "Renamed",
              description: "Updated",
              created_at: "2026-04-17T00:00:00Z",
              updated_at: "2026-04-19T00:00:00Z",
              document_count: 4,
              status_summary: { ready: 3 },
              shared: false,
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(getWorkspace("token-123", "ws-1")).resolves.toEqual({
      workspace: {
        workspace_id: "ws-1",
        org_id: "org-1",
        owner_id: "user-1",
        name: "Workspace 1",
        title: "Workspace 1",
        description: "Description",
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-18T00:00:00Z",
        document_count: 3,
        status_summary: { ready: 2 },
        shared: true,
      },
    });

    await expect(
      updateWorkspace("token-123", "ws-1", {
        name: "Workspace 1",
        description: "Updated",
      }),
    ).resolves.toEqual({
      workspace: {
        workspace_id: "ws-1",
        org_id: "org-1",
        owner_id: "user-1",
        name: "Workspace 1",
        title: "Renamed",
        description: "Updated",
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-19T00:00:00Z",
        document_count: 4,
        status_summary: { ready: 3 },
        shared: false,
      },
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/notebooks/ws-1",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/notebooks/ws-1",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          name: "Workspace 1",
          description: "Updated",
        }),
      }),
    );

    const [, init] = fetchMock.mock.calls[0]!;
    expect(new Headers(init.headers).get("Authorization")).toBe("Bearer token-123");
  });

  it("wraps sessions, sources, and notes with workspace-scoped request bodies", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            sessions: [
              {
                id: "sess-1",
                notebook_id: "ws-1",
                title: "Draft",
                agent_type: "rag",
                summary: "Summary",
                pinned: true,
                created_at: "2026-04-17T00:00:00Z",
                updated_at: "2026-04-18T00:00:00Z",
              },
            ],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: "sess-2",
            notebook_id: "ws-1",
            title: null,
            agent_type: "rag",
            summary: null,
            pinned: false,
            created_at: "2026-04-19T00:00:00Z",
            updated_at: "2026-04-19T00:00:00Z",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: "sess-2",
            notebook_id: "ws-1",
            title: "Renamed",
            agent_type: "rag",
            summary: "Updated summary",
            pinned: true,
            created_at: "2026-04-19T00:00:00Z",
            updated_at: "2026-04-20T00:00:00Z",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            id: "sess-2",
            notebook_id: "ws-1",
            title: "Renamed",
            agent_type: "rag",
            summary: "Updated summary",
            pinned: true,
            created_at: "2026-04-19T00:00:00Z",
            updated_at: "2026-04-20T00:00:00Z",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            messages: [
              {
                id: 11,
                session_id: "sess-2",
                role: "assistant",
                content: "Hello",
                answer_blocks: [{ type: "text", text: "Hello", citations: [] }],
                agent_id: "search",
                agent_name: "网络搜索助手",
                agent_icon: "🔍",
                citations: [],
                created_at: "2026-04-20T00:00:00Z",
              },
            ],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            sources: [
              {
                id: "src-1",
                notebook_id: "ws-1",
                notebook_name: "Workspace 1",
                title: "Doc",
                file_name: "alpha.pdf",
                status: "ready",
              },
            ],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            document_id: "doc-1",
            upload_url: "https://upload.example.test/doc-1",
            status: "pending",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            notes: [
              {
                id: "note-1",
                notebook_id: "ws-1",
                title: "Note",
                content: "Body",
                preview: "Body",
                created_at: "2026-04-17T00:00:00Z",
                updated_at: "2026-04-18T00:00:00Z",
                promoted_document_id: null,
                promoted_at: null,
              },
            ],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            note: {
              id: "note-2",
              notebook_id: "ws-1",
              title: "Created",
              content: "Fresh",
              preview: "Fresh",
              created_at: "2026-04-19T00:00:00Z",
              updated_at: "2026-04-19T00:00:00Z",
              promoted_document_id: null,
              promoted_at: null,
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            note: {
              id: "note-2",
              notebook_id: "ws-1",
              title: "Updated",
              content: "Fresh",
              preview: "Fresh",
              created_at: "2026-04-19T00:00:00Z",
              updated_at: "2026-04-20T00:00:00Z",
              promoted_document_id: null,
              promoted_at: null,
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            note: {
              id: "note-2",
              notebook_id: "ws-1",
              title: "Updated",
              content: "Fresh",
              preview: "Fresh",
              created_at: "2026-04-19T00:00:00Z",
              updated_at: "2026-04-20T00:00:00Z",
              promoted_document_id: "doc-2",
              promoted_at: "2026-04-20T00:00:00Z",
            },
            source_id: "src-2",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(listWorkspaceSessions("token-123", "ws-1")).resolves.toEqual({
      sessions: [
        {
          id: "sess-1",
          workspace_id: "ws-1",
          title: "Draft",
          agent_type: "rag",
          summary: "Summary",
          pinned: true,
          created_at: "2026-04-17T00:00:00Z",
          updated_at: "2026-04-18T00:00:00Z",
        },
      ],
    });
    await expect(
      createWorkspaceSession("token-123", "ws-1", {
        title: null,
        agent_type: "rag",
      }),
    ).resolves.toEqual({
      id: "sess-2",
      workspace_id: "ws-1",
      title: null,
      agent_type: "rag",
      summary: null,
      pinned: false,
      created_at: "2026-04-19T00:00:00Z",
      updated_at: "2026-04-19T00:00:00Z",
    });
    await expect(
      updateWorkspaceSession("token-123", "sess-2", {
        title: "Renamed",
        pinned: true,
      }),
    ).resolves.toEqual({
      id: "sess-2",
      workspace_id: "ws-1",
      title: "Renamed",
      agent_type: "rag",
      summary: "Updated summary",
      pinned: true,
      created_at: "2026-04-19T00:00:00Z",
      updated_at: "2026-04-20T00:00:00Z",
    });
    await expect(getWorkspaceSession("token-123", "sess-2")).resolves.toEqual({
      id: "sess-2",
      workspace_id: "ws-1",
      title: "Renamed",
      agent_type: "rag",
      summary: "Updated summary",
      pinned: true,
      created_at: "2026-04-19T00:00:00Z",
      updated_at: "2026-04-20T00:00:00Z",
    });

    await expect(listWorkspaceSessionMessages("token-123", "sess-2")).resolves.toEqual({
      messages: [
        {
          id: 11,
          session_id: "sess-2",
          role: "assistant",
          content: "Hello",
          answer_blocks: [{ type: "text", text: "Hello", citations: [] }],
          agent_id: "search",
          agent_name: "网络搜索助手",
          agent_icon: "🔍",
          citations: [],
          created_at: "2026-04-20T00:00:00Z",
        },
      ],
    });
    await expect(listWorkspaceSources("token-123", "ws-1")).resolves.toEqual({
      sources: [
        {
          id: "src-1",
          workspace_id: "ws-1",
          workspace_name: "Workspace 1",
          title: "Doc",
          file_name: "alpha.pdf",
          status: "ready",
        },
      ],
    });
    await expect(addWorkspaceSourceUrl("token-123", "ws-1", "https://example.test")).resolves.toEqual({
      document_id: "doc-1",
      upload_url: "https://upload.example.test/doc-1",
      status: "pending",
    });
    await expect(deleteWorkspaceDocument("token-123", "doc-1")).resolves.toBeUndefined();
    await expect(reindexWorkspaceDocument("token-123", "doc-1")).resolves.toBeUndefined();
    await expect(listWorkspaceNotes("token-123", "ws-1")).resolves.toEqual({
      notes: [
        {
          id: "note-1",
          workspace_id: "ws-1",
          title: "Note",
          content: "Body",
          preview: "Body",
          created_at: "2026-04-17T00:00:00Z",
          updated_at: "2026-04-18T00:00:00Z",
          promoted_document_id: null,
          promoted_at: null,
        },
      ],
    });
    await expect(
      createWorkspaceNote("token-123", "ws-1", {
        title: "Created",
        content: "Fresh",
      }),
    ).resolves.toEqual({
      note: {
        id: "note-2",
        workspace_id: "ws-1",
        title: "Created",
        content: "Fresh",
        preview: "Fresh",
        created_at: "2026-04-19T00:00:00Z",
        updated_at: "2026-04-19T00:00:00Z",
        promoted_document_id: null,
        promoted_at: null,
      },
    });
    await expect(
      updateWorkspaceNote("token-123", "ws-1", "note-2", {
        title: "Updated",
        content: "Fresh",
      }),
    ).resolves.toEqual({
      note: {
        id: "note-2",
        workspace_id: "ws-1",
        title: "Updated",
        content: "Fresh",
        preview: "Fresh",
        created_at: "2026-04-19T00:00:00Z",
        updated_at: "2026-04-20T00:00:00Z",
        promoted_document_id: null,
        promoted_at: null,
      },
    });
    await expect(deleteWorkspaceNote("token-123", "ws-1", "note-2")).resolves.toBeUndefined();
    await expect(promoteWorkspaceNote("token-123", "ws-1", "note-2")).resolves.toEqual({
      note: {
        id: "note-2",
        workspace_id: "ws-1",
        title: "Updated",
        content: "Fresh",
        preview: "Fresh",
        created_at: "2026-04-19T00:00:00Z",
        updated_at: "2026-04-20T00:00:00Z",
        promoted_document_id: "doc-2",
        promoted_at: "2026-04-20T00:00:00Z",
      },
      source_id: "src-2",
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/chat/sessions?notebook_id=ws-1",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/chat/sessions",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          notebook_id: "ws-1",
          title: null,
          agent_type: "rag",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "https://api.example.test/api/v1/chat/sessions/sess-2",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      5,
      "https://api.example.test/api/v1/chat/sessions/sess-2/messages",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      "https://api.example.test/api/v1/sources?notebook_id=ws-1",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      7,
      "https://api.example.test/api/v1/notebooks/ws-1/sources/url",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          url: "https://example.test",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      8,
      "https://api.example.test/api/v1/documents/doc-1",
      expect.objectContaining({
        method: "DELETE",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      9,
      "https://api.example.test/api/v1/documents/doc-1/reindex",
      expect.objectContaining({
        method: "POST",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      10,
      "https://api.example.test/api/v1/notebooks/ws-1/notes",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      11,
      "https://api.example.test/api/v1/notebooks/ws-1/notes",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          title: "Created",
          content: "Fresh",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      12,
      "https://api.example.test/api/v1/notebooks/ws-1/notes/note-2",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          title: "Updated",
          content: "Fresh",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      13,
      "https://api.example.test/api/v1/notebooks/ws-1/notes/note-2",
      expect.objectContaining({
        method: "DELETE",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      14,
      "https://api.example.test/api/v1/notebooks/ws-1/notes/note-2/promote-to-source",
      expect.objectContaining({
        method: "POST",
      }),
    );
  });

  it("creates, uploads, and completes document uploads", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            document_id: "doc-1",
            upload_url: "https://upload.example.test/uploads/doc-1",
            status: "pending",
          }),
          {
            status: 201,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ status: "uploaded" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );

    await expect(
      createWorkspaceDocumentUpload("token-123", "ws-1", {
        filename: "notes.md",
        file_size: 11,
        mime_type: "text/markdown",
      }),
    ).resolves.toEqual({
      document_id: "doc-1",
      upload_url: "https://upload.example.test/uploads/doc-1",
      status: "pending",
    });

    const file = new File(["hello world"], "notes.md", { type: "text/markdown" });
    await expect(uploadWorkspaceDocumentFile("https://upload.example.test/uploads/doc-1", file)).resolves.toBeUndefined();
    await expect(completeWorkspaceDocumentUpload("token-123", "doc-1")).resolves.toBeUndefined();

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/notebooks/ws-1/documents",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          filename: "notes.md",
          file_size: 11,
          mime_type: "text/markdown",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://upload.example.test/uploads/doc-1",
      expect.objectContaining({
        method: "PUT",
        body: file,
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/v1/documents/doc-1/complete-upload",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({}),
      }),
    );
  });
});
