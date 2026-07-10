import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createWorkspace,
  deleteWorkspace,
  listWorkspaces,
  updateWorkspace,
} from "../../lib/dashboard/client";

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

describe("dashboard client", () => {
  it("lists workspaces through the workspace transport contract", async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          workspaces: [
            {
              id: "ws-1",
              owner_user_id: "owner-1",
              owner_id: "user-1",
              name: "Workspace 1",
              title: "Workspace 1",
              description: "Desc",
              created_at: "2026-04-17T00:00:00Z",
              updated_at: "2026-04-17T00:00:00Z",
              document_count: 3,
              status_summary: { ready: 2 },
              shared: true,
            },
          ],
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    await expect(listWorkspaces("token-123")).resolves.toEqual({
      workspaces: [
        {
          workspace_id: "ws-1",
          owner_user_id: "owner-1",
          owner_id: "user-1",
          name: "Workspace 1",
          title: "Workspace 1",
          description: "Desc",
          created_at: "2026-04-17T00:00:00Z",
          updated_at: "2026-04-17T00:00:00Z",
          document_count: 3,
          status_summary: { ready: 2 },
          shared: true,
        },
      ],
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/workspaces",
      expect.objectContaining({
        method: "GET",
        cache: "no-store",
      }),
    );

    const [, init] = fetchMock.mock.calls[0]!;
    const headers = new Headers(init.headers);

    expect(headers.get("Authorization")).toBe("Bearer token-123");
  });

  it("creates, updates, and deletes workspaces with workspace_id at the boundary", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            workspace: {
              id: "ws-2",
              owner_user_id: "owner-1",
              owner_id: "user-1",
              name: "New Workspace",
              title: "New Workspace",
              description: "",
              created_at: "2026-04-17T00:00:00Z",
              updated_at: "2026-04-17T00:00:00Z",
              document_count: 0,
              status_summary: {},
              shared: false,
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
            workspace: {
              id: "ws-2",
              owner_user_id: "owner-1",
              owner_id: "user-1",
              name: "Renamed Workspace",
              title: "Renamed Workspace",
              description: "Updated description",
              created_at: "2026-04-17T00:00:00Z",
              updated_at: "2026-04-18T00:00:00Z",
              document_count: 1,
              status_summary: { ready: 1 },
              shared: false,
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
      );

    await expect(
      createWorkspace("token-123", {
        name: "New Workspace",
        description: "",
      }),
    ).resolves.toEqual({
      workspace: {
        workspace_id: "ws-2",
        owner_user_id: "owner-1",
        owner_id: "user-1",
        name: "New Workspace",
        title: "New Workspace",
        description: "",
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-17T00:00:00Z",
        document_count: 0,
        status_summary: {},
        shared: false,
      },
    });

    await expect(
      updateWorkspace("token-123", "ws-2", {
        name: "Renamed Workspace",
        description: "Updated description",
      }),
    ).resolves.toEqual({
      workspace: {
        workspace_id: "ws-2",
        owner_user_id: "owner-1",
        owner_id: "user-1",
        name: "Renamed Workspace",
        title: "Renamed Workspace",
        description: "Updated description",
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-18T00:00:00Z",
        document_count: 1,
        status_summary: { ready: 1 },
        shared: false,
      },
    });

    await expect(deleteWorkspace("token-123", "ws-2")).resolves.toBeUndefined();

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/workspaces",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          name: "New Workspace",
          description: "",
        }),
      }),
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/workspaces/ws-2",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          name: "Renamed Workspace",
          description: "Updated description",
        }),
      }),
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/v1/workspaces/ws-2",
      expect.objectContaining({
        method: "DELETE",
      }),
    );
  });
});
