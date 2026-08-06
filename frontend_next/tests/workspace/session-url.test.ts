import { describe, expect, it } from "vitest";

import {
  sessionQueryMatches,
  workspaceSessionHref,
} from "../../lib/workspace/session-url";

describe("workspaceSessionHref", () => {
  it("omits the query when no session is selected", () => {
    expect(workspaceSessionHref("ws-1", null)).toBe("/dashboard/ws-1");
    expect(workspaceSessionHref("ws-1", undefined)).toBe("/dashboard/ws-1");
    expect(workspaceSessionHref("ws-1", "  ")).toBe("/dashboard/ws-1");
  });

  it("encodes the session id in the query string", () => {
    expect(workspaceSessionHref("ws-1", "sess-9")).toBe("/dashboard/ws-1?session=sess-9");
    expect(workspaceSessionHref("ws-1", "a b")).toBe("/dashboard/ws-1?session=a%20b");
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
