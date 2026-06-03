import { test as base } from "@playwright/test";
import { readRunId } from "../utils/api-helpers";

/**
 * 扩展 Playwright test，注入 runId fixture。
 *
 * 用法：
 *   import { test, expect } from "../fixtures/run-context";
 *   test("...", async ({ page, runId }) => { ... });
 *
 * 设计理由：runId 是测试运行的上下文信息，不是业务数据。
 * 通过 fixture 注入比每个 spec 手动调用 readRunId() 更清晰，
 * 也便于未来扩展（如 worker-scoped runId、并行账号分配等）。
 */
export const test = base.extend<{
  runId: string;
}>({
  runId: async ({}, use) => {
    await use(readRunId());
  },
});

export { expect } from "@playwright/test";
