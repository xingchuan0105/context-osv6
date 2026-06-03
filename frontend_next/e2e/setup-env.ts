import { request } from "@playwright/test";
import { resetTestUserData } from "./utils/api-helpers";

/**
 * setup-env 职责：纯环境准备，无浏览器参与。
 *   1. 生成 runId 并持久化到磁盘（供所有 spec 和后续 setup-auth 复用）
 *   2. 重置预置账号数据（清空上一 run 的残留）
 *
 * 设计理由：这一步完全不需要浏览器。如果 runId 生成或 reset 失败，
 * 失败原因明确归为"环境/后端"问题，而不是"登录流程"问题。
 */
export default async function setupEnv() {
  const runId = `r${Date.now()}`;

  const fs = await import("fs");
  fs.mkdirSync("playwright/.auth", { recursive: true });
  fs.writeFileSync("playwright/.auth/run-id.txt", runId);

  console.log(`[setup-env] runId generated: ${runId}`);

  // 使用 Playwright 的 request 对象（无需浏览器）调用 reset API
  const reqCtx = await request.newContext({
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
  });

  try {
    await resetTestUserData(reqCtx);
    console.log("[setup-env] reset-user-data succeeded");
  } catch (e) {
    console.error("[setup-env] reset-user-data failed:", e);
    throw e;
  } finally {
    await reqCtx.dispose();
  }
}
