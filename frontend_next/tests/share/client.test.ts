import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  buildShareUrl,
  getShareAccessLogs,
  getShareAnalytics,
  getShareSettings,
  getSharedWorkspace,
  isShareEnabled,
} from "../../lib/share/client";

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

describe("share client", () => {
  it("maps share settings to the active share token boundary", async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          access_level: "link",
          allow_download: true,
          share_tokens: [
            {
              token: "revoked-token",
              access_level: "read",
              revoked_at: "2026-04-01T00:00:00Z",
            },
            {
              token: "active-token",
              access_level: "read",
              expires_at: "2026-04-30T18:00:00Z",
              revoked_at: null,
            },
          ],
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    await expect(getShareSettings("token-123", "ws-1")).resolves.toEqual({
      share_token: "active-token",
      access_level: "link",
      expires_at: "2026-04-30T18:00:00Z",
      allow_download: true,
    });

    expect(isShareEnabled({
      share_token: "active-token",
      access_level: "link",
      expires_at: null,
      allow_download: false,
    })).toBe(true);

    expect(buildShareUrl("active-token")).toContain("/shared/kb/active-token");
  });

  it("aggregates analytics and access logs from the raw backend envelopes", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: [
              {
                token: "share-1",
                access_level: "read",
                total_views: 3,
                created_at: "2026-04-17T09:00:00Z",
              },
              {
                token: "share-2",
                access_level: "read",
                total_views: 2,
                created_at: "2026-04-17T15:00:00Z",
              },
              {
                token: "share-3",
                access_level: "write",
                total_views: 4,
                created_at: "2026-04-18T09:00:00Z",
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
            ok: true,
            data: [
              {
                id: "log-1",
                notebook_id: "ws-1",
                share_token: "visitor-a",
                action: "view",
                accessed_at: 1713369600,
              },
            ],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(getShareAnalytics("token-123", "ws-1")).resolves.toEqual({
      total_views: 9,
      total_unique_visitors: 3,
      views_by_day: {
        "2026-04-17": 5,
        "2026-04-18": 4,
      },
    });

    await expect(getShareAccessLogs("token-123", "ws-1")).resolves.toEqual({
      logs: [
        {
          id: "log-1",
          visitor_id: "visitor-a",
          accessed_at: "1713369600",
          action: "view",
        },
      ],
    });
  });

  it("unwraps the public shared workspace payload and rejects invalid links", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            success: true,
            data: {
              knowledge_base: {
                id: "ws-1",
                title: "Shared Workspace",
                description: "Shared description",
              },
              share: {
                permission: "partial",
                expires_at: null,
                allow_download: false,
                scope: "partial",
              },
              sources: [
                {
                  id: "src-1",
                  file_name: "Plan.pdf",
                  status: "ready",
                },
              ],
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
            success: false,
            error: "invalid share token",
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(getSharedWorkspace("share-123")).resolves.toEqual({
      knowledge_base: {
        id: "ws-1",
        title: "Shared Workspace",
        description: "Shared description",
      },
      share: {
        permission: "partial",
        expires_at: null,
        allow_download: false,
        scope: "partial",
      },
      sources: [
        {
          id: "src-1",
          file_name: "Plan.pdf",
          status: "ready",
        },
      ],
    });

    await expect(getSharedWorkspace("expired-token")).rejects.toThrow("invalid share token");
  });
});
