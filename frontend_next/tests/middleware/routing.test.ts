import { describe, expect, it } from "vitest";

import { resolveMiddlewareAction } from "../../lib/middleware-routing";

describe("resolveMiddlewareAction", () => {
  it("applies PRD compatibility redirects", () => {
    expect(resolveMiddlewareAction("/dashboard/search", true)).toEqual({
      type: "redirect",
      destination: "/dashboard",
    });
    expect(resolveMiddlewareAction("/workspaces/ws-1/share", true)).toEqual({
      type: "redirect",
      destination: "/dashboard/ws-1/share",
    });
  });

  it("does not redirect retired admin org paths (gone, not aliased)", () => {
    expect(resolveMiddlewareAction("/admin/orgs/abc", true)).toEqual({ type: "next" });
    expect(resolveMiddlewareAction("/admin/organizations", true)).toEqual({ type: "next" });
    expect(resolveMiddlewareAction("/admin/accounts/abc", true)).toEqual({ type: "next" });
  });

  it("allows public paths to continue", () => {
    expect(resolveMiddlewareAction("/help", false)).toEqual({ type: "next" });
    expect(resolveMiddlewareAction("/invite/ws-1/member-1", false)).toEqual({ type: "next" });
    expect(resolveMiddlewareAction("/login", false)).toEqual({ type: "next" });
    expect(resolveMiddlewareAction("/dashboard", false)).toEqual({ type: "next" });
  });

  it("redirects legacy agent api doc paths to /help/api-access/agents", () => {
    expect(resolveMiddlewareAction("/docs/api-access-for-agents.md", false)).toEqual({
      type: "redirect",
      destination: "/help/api-access/agents",
    });
    expect(resolveMiddlewareAction("/docs/api-access-for-agents", true)).toEqual({
      type: "redirect",
      destination: "/help/api-access/agents",
    });
  });
});
