import { type APIRequestContext } from "@playwright/test";
import { TEST_USER } from "../fixtures/test-user";

/**
 * 重置预置测试账号的所有数据。
 * 仅用于 setup/teardown，不用于测试断言。
 */
export async function resetTestUserData(request: APIRequestContext) {
  const resp = await request.post("/api/e2e/reset-user-data", {
    headers: { "X-E2E-Secret": process.env.E2E_RESET_SECRET! },
    data: { email: TEST_USER.email },
  });
  if (!resp.ok()) {
    throw new Error(`reset-user-data failed: ${resp.status()} ${await resp.text()}`);
  }
}

/**
 * 读取 setup-env.ts 生成的 runId（run-scoped 隔离标识）。
 *
 * 注意：spec 中优先通过 run-context fixture 注入 runId，而非直接调用本函数。
 * 本函数保留作为底层工具，供 fixture 和 setup 文件使用。
 */
export function readRunId(): string {
  const fs = require("fs");
  return fs.readFileSync("playwright/.auth/run-id.txt", "utf-8").trim();
}

/**
 * 为 notebook/session 等命名，确保 run-scoped 隔离。
 */
export function runScopedName(label: string, runId: string): string {
  return `${label} ${runId}`;
}

/**
 * 通过API删除指定notebook。
 * 仅用于 afterAll 清理，不用于测试断言。
 *
 * 依赖：Playwright的 `request` fixture会自动携带storageState中的认证cookie。
 * 如果后端使用JWT Bearer Token而非session cookie，需在setup-env.ts中将token
 * 写入文件（如 `playwright/.auth/token.txt`），本函数读取后附加到header。
 */
export async function deleteNotebookViaAPI(
  request: APIRequestContext,
  notebookId: string,
) {
  if (!notebookId) return;
  await request.delete(`/api/v1/notebooks/${notebookId}`);
}
