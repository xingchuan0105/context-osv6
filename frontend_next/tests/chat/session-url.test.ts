import { describe, expect, it } from "vitest";

import { conversationHref } from "@/lib/chat/session-url";

describe("conversationHref", () => {
  it("routes personal conversations to the chat canonical", () => {
    expect(conversationHref({ id: "personal / 1", workspace_id: null })).toBe(
      "/chat/personal%20%2F%201",
    );
    expect(conversationHref({ id: "personal-2" })).toBe("/chat/personal-2");
  });

  it("routes Workspace conversations to the Workspace canonical", () => {
    expect(conversationHref({ id: "session / 1", workspace_id: "ws-1" })).toBe(
      "/dashboard/ws-1?session=session%20%2F%201",
    );
  });
});
