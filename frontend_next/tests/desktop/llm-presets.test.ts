import { describe, expect, it } from "vitest";

import { findLlmPreset, LLM_PRESETS } from "@/lib/desktop/llm-presets";

describe("llm-presets", () => {
  it("defines 16 provider presets including custom", () => {
    expect(LLM_PRESETS).toHaveLength(16);
  });

  it("includes featured providers from the design doc", () => {
    const ids = LLM_PRESETS.map((preset) => preset.id);

    expect(ids).toContain("zhipu");
    expect(ids).toContain("anthropic");
    expect(ids).toContain("deepseek");
    expect(ids).toContain("ollama");
    expect(ids).toContain("custom");
  });

  it("finds presets by id", () => {
    const zhipu = findLlmPreset("zhipu");

    expect(zhipu?.base_url).toBe("https://open.bigmodel.cn/api/paas/v4");
    expect(zhipu?.model).toBe("glm-4.6");
  });
});
