import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createApiKey,
  getApiAccessBaseUrl,
  listApiKeys,
  revokeApiKey,
} from "../../lib/api-access/client";

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

describe("api access client", () => {
  it("uses notebook-scoped api key endpoints for list, create, and revoke", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            api_keys: [
              {
                id: "key-1",
                owner_user_id: "owner-1",
                workspace_id: "ws-1",
                key_prefix: "sk_live_123",
                name: "Indexer",
                permissions: ["index"],
                rate_limit_rpm: 60,
                expires_at: null,
                last_used_at: null,
                is_active: true,
                created_by: "user-1",
                created_at: "2026-04-17T10:00:00Z",
                updated_at: "2026-04-17T10:00:00Z",
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
            api_key: {
              id: "key-2",
              owner_user_id: "owner-1",
              workspace_id: "ws-1",
              key_prefix: "sk_live_456",
              name: "Agent Key",
              permissions: ["index", "query"],
              rate_limit_rpm: 120,
              expires_at: "2026-05-01T00:00:00Z",
              last_used_at: null,
              is_active: true,
              created_by: "user-1",
              created_at: "2026-04-17T11:00:00Z",
              updated_at: "2026-04-17T11:00:00Z",
            },
            plaintext_key: "sk_workspace_plaintext",
          }),
          {
            status: 201,
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

    await expect(listApiKeys("token-123", "ws-1")).resolves.toEqual({
      api_keys: [
        {
          id: "key-1",
          owner_user_id: "owner-1",
          workspace_id: "ws-1",
          key_prefix: "sk_live_123",
          name: "Indexer",
          permissions: ["index"],
          rate_limit_rpm: 60,
          expires_at: null,
          last_used_at: null,
          is_active: true,
          created_by: "user-1",
          created_at: "2026-04-17T10:00:00Z",
          updated_at: "2026-04-17T10:00:00Z",
        },
      ],
    });

    await expect(
      createApiKey("token-123", "ws-1", {
        name: "Agent Key",
        permissions: ["index", "query"],
        rate_limit_rpm: 120,
        expires_at: "2026-05-01T00:00:00Z",
      }),
    ).resolves.toEqual({
      api_key: {
        id: "key-2",
        owner_user_id: "owner-1",
        workspace_id: "ws-1",
        key_prefix: "sk_live_456",
        name: "Agent Key",
        permissions: ["index", "query"],
        rate_limit_rpm: 120,
        expires_at: "2026-05-01T00:00:00Z",
        last_used_at: null,
        is_active: true,
        created_by: "user-1",
        created_at: "2026-04-17T11:00:00Z",
        updated_at: "2026-04-17T11:00:00Z",
      },
      plaintext_key: "sk_workspace_plaintext",
    });

    await expect(revokeApiKey("token-123", "ws-1", "key-2")).resolves.toEqual({});

    expect(getApiAccessBaseUrl()).toBe("https://api.example.test");
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/workspaces/ws-1/api-keys",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/workspaces/ws-1/api-keys",
      expect.objectContaining({
        method: "POST",
      }),
    );
    expect(fetchMock.mock.calls[1]?.[1]).toEqual(
      expect.objectContaining({
        body: JSON.stringify({
          name: "Agent Key",
          permissions: ["index", "query"],
          rate_limit_rpm: 120,
          expires_at: "2026-05-01T00:00:00Z",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/v1/workspaces/ws-1/api-keys/key-2",
      expect.objectContaining({
        method: "DELETE",
      }),
    );
  });
});
