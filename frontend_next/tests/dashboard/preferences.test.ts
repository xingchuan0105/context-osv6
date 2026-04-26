import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  getFavoriteWorkspaceIds,
  updateFavoriteWorkspaceIds,
} from "../../lib/dashboard/preferences";

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

describe("dashboard preferences", () => {
  it("reads favorite workspace ids from the auth preferences contract", async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          dashboard: {
            favorite_notebook_ids: ["ws-1", "ws-2"],
            workspace_drafts: [],
            workspace_preferences: [],
            notebook_notes: [],
          },
          notifications: {},
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    await expect(getFavoriteWorkspaceIds("token-123")).resolves.toEqual(["ws-1", "ws-2"]);

    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/auth/preferences",
      expect.objectContaining({
        method: "GET",
        cache: "no-store",
      }),
    );

    const [, init] = fetchMock.mock.calls[0]!;
    const headers = new Headers(init.headers);
    expect(headers.get("Authorization")).toBe("Bearer token-123");
  });

  it("preserves the rest of the dashboard preferences when updating favorites", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            dashboard: {
              favorite_notebook_ids: ["ws-1"],
              workspace_drafts: [{ notebook_id: "ws-3", notes: "draft" }],
              workspace_preferences: [{ notebook_id: "ws-4", pinned_source_ids: ["src-1"] }],
              notebook_notes: [],
            },
            notifications: {},
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
            dashboard: {
              favorite_notebook_ids: ["ws-2", "ws-3"],
              workspace_drafts: [{ notebook_id: "ws-3", notes: "draft" }],
              workspace_preferences: [{ notebook_id: "ws-4", pinned_source_ids: ["src-1"] }],
              notebook_notes: [],
            },
            notifications: {},
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(updateFavoriteWorkspaceIds("token-123", ["ws-2", "ws-3"])).resolves.toEqual([
      "ws-2",
      "ws-3",
    ]);

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/auth/preferences",
      expect.objectContaining({
        method: "GET",
      }),
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/auth/preferences",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          dashboard: {
            favorite_notebook_ids: ["ws-2", "ws-3"],
            workspace_drafts: [{ notebook_id: "ws-3", notes: "draft" }],
            workspace_preferences: [{ notebook_id: "ws-4", pinned_source_ids: ["src-1"] }],
            notebook_notes: [],
          },
          notifications: {},
        }),
      }),
    );

    const [, init] = fetchMock.mock.calls[1]!;
    const headers = new Headers(init.headers);
    expect(headers.get("Authorization")).toBe("Bearer token-123");
  });
});
