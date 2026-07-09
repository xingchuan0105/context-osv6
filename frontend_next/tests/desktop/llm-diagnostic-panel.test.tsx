import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@/lib/desktop/tauri-license", () => ({
  openInBrowser: vi.fn(),
}));

import { LLMDiagnosticPanel } from "@/components/desktop/LLMDiagnosticPanel";

const config = {
  provider: "zhipu",
  base_url: "https://open.bigmodel.cn/api/paas/v4",
  api_key: "test-key",
  model: "glm-4.6",
  timeout_ms: 30_000,
};

describe("LLMDiagnosticPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("runsDiagnosticAndRendersChecks", async () => {
    invokeMock.mockResolvedValueOnce({
      overall: "error",
      checks: [{ name: "model_available", status: "error", message: "模型不存在" }],
      suggestions: [
        {
          code: "switch_model",
          message: "切换模型",
          action: { type: "UpdateConfig", patch: { model: "glm-4-plus" } },
        },
      ],
    });

    render(<LLMDiagnosticPanel config={config} />);

    fireEvent.click(screen.getByRole("button", { name: "运行诊断" }));

    await waitFor(() => {
      expect(screen.getByText(/模型不存在/)).toBeInTheDocument();
    });

    expect(screen.getByRole("button", { name: "使用 glm-4-plus" })).toBeInTheDocument();
  });

  it("disablesDiagnosticWhenConfigMissing", () => {
    render(<LLMDiagnosticPanel />);

    expect(screen.getByRole("button", { name: "运行诊断" })).toBeDisabled();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
