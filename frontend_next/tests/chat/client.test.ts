import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  getChatSession,
  listChatSessions,
  sortConversationsByRecent,
  type ConversationSummary,
} from "@/lib/chat/client";
import { ConversationScopeKind } from "@/lib/contracts/generated";

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

describe("chat client", () => {
  it("lists every owner-visible conversation without a workspace filter", async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          sessions: [
            {
              id: "personal-1",
              owner_user_id: "user-1",
              scope_kind: ConversationScopeKind.Personal,
              title: "Personal chat",
              agent_type: "chat",
              model_role: "quick_chat",
              pinned: false,
              created_at: "2026-09-01T00:00:00Z",
              updated_at: "2026-09-02T00:00:00Z",
            },
          ],
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    const response = await listChatSessions("token-123");
    expect(response).toMatchObject({
      sessions: [{ id: "personal-1" }],
    });
    expect(response.sessions[0]).not.toHaveProperty("workspace_id");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/chat/sessions",
      expect.objectContaining({ method: "GET", cache: "no-store" }),
    );
    expect(fetchMock.mock.calls[0]?.[0]).not.toContain("workspace_id");
  });

  it("loads one exact owner-scoped conversation before a deep link is rendered", async () => {
    fetchMock.mockResolvedValue(
      new Response(
        JSON.stringify({
          id: "personal / 1",
          owner_user_id: "user-1",
          scope_kind: ConversationScopeKind.Personal,
          title: "Personal chat",
          agent_type: "chat",
          model_role: "quick_chat",
          pinned: false,
          created_at: "2026-09-01T00:00:00Z",
          updated_at: "2026-09-02T00:00:00Z",
        }),
        {
          status: 200,
          headers: { "Content-Type": "application/json" },
        },
      ),
    );

    const response = await getChatSession("token-123", "personal / 1");
    expect(response).toMatchObject({
      id: "personal / 1",
    });
    expect(response).not.toHaveProperty("workspace_id");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/chat/sessions/personal%20%2F%201",
      expect.objectContaining({ method: "GET", cache: "no-store" }),
    );
  });

  it("orders global recents by update time and ignores workspace pinning", () => {
    const base = {
      owner_user_id: "user-1",
      scope_kind: ConversationScopeKind.Personal,
      agent_type: "chat",
      model_role: "quick_chat",
      created_at: "2026-09-01T00:00:00Z",
    };
    const sessions: ConversationSummary[] = [
      {
        ...base,
        id: "workspace-old",
        workspace_id: "ws-1",
        scope_kind: ConversationScopeKind.Workspace,
        pinned: true,
        updated_at: "2026-09-01T00:00:00Z",
      },
      {
        ...base,
        id: "personal-new",
        pinned: false,
        updated_at: "2026-09-02T00:00:00Z",
      },
      {
        ...base,
        id: "a-tie",
        updated_at: "2026-09-01T00:00:00Z",
      },
    ];

    expect(sortConversationsByRecent(sessions).map((session) => session.id)).toEqual([
      "personal-new",
      "a-tie",
      "workspace-old",
    ]);
  });
});
