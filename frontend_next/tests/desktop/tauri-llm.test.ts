import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@/lib/desktop/tauri-license", () => ({
  openInBrowser: vi.fn(),
}));

import { openInBrowser } from "@/lib/desktop/tauri-license";
import {
  diagnoseLlm,
  executeRepairAction,
  mergeLlmConfigPatch,
  repairActionLabel,
  setLlmConfig,
  testLlmConnection,
} from "@/lib/desktop/tauri-llm";

describe("tauri-llm", () => {
  const config = {
    provider: "zhipu",
    base_url: "https://open.bigmodel.cn/api/paas/v4",
    api_key: "test-key",
    model: "glm-4.6",
    timeout_ms: 30_000,
  };

  beforeEach(() => {
    invokeMock.mockReset();
    vi.mocked(openInBrowser).mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("testLlmConnection_invokesTestCommand", async () => {
    invokeMock.mockResolvedValueOnce({ ok: true, latency_ms: 120, message: "ok" });

    const result = await testLlmConnection(config);

    expect(result.ok).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith("test_llm_connection", { config });
  });

  it("setLlmConfig_invokesSetCommand", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await setLlmConfig(config);

    expect(invokeMock).toHaveBeenCalledWith("set_llm_config", { config });
  });

  it("diagnoseLlm_returnsDiagnosticReport", async () => {
    invokeMock.mockResolvedValueOnce({
      overall: "warning",
      checks: [{ name: "dns", status: "ok", message: "ok" }],
      suggestions: [],
    });

    const report = await diagnoseLlm(config);

    expect(report.overall).toBe("warning");
    expect(report.checks).toHaveLength(1);
  });

  it("mergeLlmConfigPatch_mergesModelPatch", () => {
    const merged = mergeLlmConfigPatch(config, { model: "glm-4-plus" });

    expect(merged.model).toBe("glm-4-plus");
    expect(merged.provider).toBe("zhipu");
  });

  it("executeRepairAction_openUrl_callsOpenInBrowser", async () => {
    const result = await executeRepairAction({
      type: "OpenUrl",
      url: "https://example.com/key",
    });

    expect(openInBrowser).toHaveBeenCalledWith("https://example.com/key");
    expect(result.applied).toBe(true);
  });

  it("executeRepairAction_updateConfig_persistsMergedConfig", async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    const result = await executeRepairAction(
      { type: "UpdateConfig", patch: { model: "glm-4-plus" } },
      { currentConfig: config },
    );

    expect(invokeMock).toHaveBeenCalledWith("set_llm_config", {
      config: expect.objectContaining({ model: "glm-4-plus" }),
    });
    expect(result.applied).toBe(true);
  });

  it("repairActionLabel_formatsUpdateConfigLabel", () => {
    expect(
      repairActionLabel({ type: "UpdateConfig", patch: { model: "glm-4-plus" } }),
    ).toBe("使用 glm-4-plus");
  });
});
