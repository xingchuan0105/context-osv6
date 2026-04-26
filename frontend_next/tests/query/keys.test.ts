import { describe, expect, it } from "vitest";

import { queryKeys } from "../../lib/query/keys";

describe("queryKeys", () => {
  it("builds stable keys for auth and dashboard queries", () => {
    expect(queryKeys.auth.me()).toEqual(["auth", "me"]);
    expect(queryKeys.dashboard.workspaces()).toEqual(["dashboard", "workspaces"]);
  });

  it("scopes workspace keys by workspace and session identifiers", () => {
    expect(queryKeys.workspace.detail("ws-1")).toEqual(["workspace", "ws-1"]);
    expect(queryKeys.workspace.sources("ws-1")).toEqual(["workspace", "ws-1", "sources"]);
    expect(queryKeys.workspace.messages("sess-1")).toEqual([
      "workspace",
      "session",
      "sess-1",
      "messages",
    ]);
  });
});
