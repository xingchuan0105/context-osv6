import { describe, expect, it } from "vitest";

import {
  AGENT_DOCS_PATH,
  HUMAN_DOCS_PATH,
  absoluteDocUrl,
  buildAgentPack,
} from "../../lib/api-access/agent-pack";

describe("buildAgentPack", () => {
  it("includes workspace_id, absolute docs, and real key when provided", () => {
    const pack = buildAgentPack({
      workspaceId: "550e8400-e29b-41d4-a716-446655440000",
      apiBase: "https://app.contextlm.top",
      apiKey: "cos_ws_secret",
      permissions: ["index", "query"],
      docsOrigin: "https://app.contextlm.top",
      runtime: "cloud",
    });

    expect(pack).toContain("workspace_id: 550e8400-e29b-41d4-a716-446655440000");
    expect(pack).toContain("api_key: cos_ws_secret");
    expect(pack).toContain("CONTEXT_OS_API_KEY\": \"cos_ws_secret\"");
    expect(pack).toContain(
      "CONTEXT_OS_WORKSPACE_ID\": \"550e8400-e29b-41d4-a716-446655440000\"",
    );
    expect(pack).toContain("mcp_http: https://app.contextlm.top/api/v1/mcp");
    expect(pack).toContain(`human: https://app.contextlm.top${HUMAN_DOCS_PATH}`);
    expect(pack).toContain(`agent: https://app.contextlm.top${AGENT_DOCS_PATH}`);
    expect(pack).toContain("status: ready");
    expect(pack).not.toContain("<CREATE_KEY_FIRST>");
  });

  it("marks missing_key without inventing a usable secret", () => {
    const pack = buildAgentPack({
      workspaceId: "550e8400-e29b-41d4-a716-446655440000",
      apiBase: "https://app.contextlm.top",
      apiKey: null,
      docsOrigin: "https://app.contextlm.top",
    });

    expect(pack).toContain("status: missing_key");
    expect(pack).toContain("<CREATE_KEY_FIRST>");
    expect(pack).not.toContain("status: ready");
  });

  it("builds absolute doc urls", () => {
    expect(absoluteDocUrl("https://app.contextlm.top/", AGENT_DOCS_PATH)).toBe(
      "https://app.contextlm.top/help/api-access/agents",
    );
  });
});
