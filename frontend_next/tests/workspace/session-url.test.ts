import { describe, expect, it } from "vitest";

import {
  sessionQueryMatches,
  workspaceDeepLinkHref,
  workspaceSessionHref,
  workspaceSourceHref,
} from "../../lib/workspace/session-url";

describe("workspaceDeepLinkHref", () => {
  it("omits the query when nothing is selected", () => {
    expect(workspaceDeepLinkHref("ws-1")).toBe("/dashboard/ws-1");
    expect(workspaceDeepLinkHref("ws-1", { sessionId: null, sourceId: "  " })).toBe(
      "/dashboard/ws-1",
    );
  });

  it("encodes session and source independently or together", () => {
    expect(workspaceDeepLinkHref("ws-1", { sessionId: "sess-9" })).toBe(
      "/dashboard/ws-1?session=sess-9",
    );
    expect(workspaceDeepLinkHref("ws-1", { sourceId: "src-1" })).toBe(
      "/dashboard/ws-1?source=src-1",
    );
    expect(
      workspaceDeepLinkHref("ws-1", { sessionId: "sess-9", sourceId: "src a" }),
    ).toBe("/dashboard/ws-1?session=sess-9&source=src%20a");
  });
});

describe("workspaceSessionHref", () => {
  it("is a session-only deep-link", () => {
    expect(workspaceSessionHref("ws-1", null)).toBe("/dashboard/ws-1");
    expect(workspaceSessionHref("ws-1", "sess-9")).toBe("/dashboard/ws-1?session=sess-9");
  });
});

describe("workspaceSourceHref", () => {
  it("builds source deep-links with optional session", () => {
    expect(workspaceSourceHref("ws-1", "src-1")).toBe("/dashboard/ws-1?source=src-1");
    expect(workspaceSourceHref("ws-1", "src-1", "sess-2")).toBe(
      "/dashboard/ws-1?session=sess-2&source=src-1",
    );
  });
});

describe("sessionQueryMatches", () => {
  it("treats empty and null as the same (no selection)", () => {
    expect(sessionQueryMatches(null, null)).toBe(true);
    expect(sessionQueryMatches("", null)).toBe(true);
    expect(sessionQueryMatches(null, "")).toBe(true);
  });

  it("matches only when both sides equal after trim", () => {
    expect(sessionQueryMatches("sess-1", "sess-1")).toBe(true);
    expect(sessionQueryMatches(" sess-1 ", "sess-1")).toBe(true);
    expect(sessionQueryMatches("sess-1", "sess-2")).toBe(false);
    expect(sessionQueryMatches("sess-1", null)).toBe(false);
  });
});
