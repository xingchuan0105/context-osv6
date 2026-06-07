import { request } from "@playwright/test";
import { resetTestUserData } from "./utils/api-helpers";

/**
 * setup-env 职责：纯环境准备，无浏览器参与。
 *   1. 生成 runId 并持久化到磁盘（供所有 spec 和后续 setup-auth 复用）
 *   2. 重置预置账号数据（清空上一 run 的残留）
 *   3. 确保 billing E2E 启用 pricing revamp 前端门控
 *
 * 设计理由：这一步完全不需要浏览器。如果 runId 生成或 reset 失败，
 * 失败原因明确归为"环境/后端"问题，而不是"登录流程"问题。
 *
 * 后端灰度：Playwright webServer 启动 avrag-api 时需 PRICING_REVAMP_ROLLOUT=100，
 * 否则预置 E2E 用户可能不在 hash 桶内而收到 feature_disabled。
 */
const RUN_ID_MAX_AGE_MS = 10 * 60 * 1000; // 10 分钟

export default async function setupEnv() {
  process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED = "1";

  const runId = `r${Date.now()}`;
  const authDir = "playwright/.auth";
  const runIdPath = `${authDir}/run-id.txt`;

  const fs = await import("fs");
  fs.mkdirSync(authDir, { recursive: true });

  // P1: 检测已有 runId，若存在且非 stale（< 10 分钟）则覆盖并警告
  if (fs.existsSync(runIdPath)) {
    const existing = fs.readFileSync(runIdPath, "utf-8").trim();
    const existingTs = parseInt(existing.slice(1), 10);
    const ageMs = Date.now() - existingTs;
    if (ageMs < RUN_ID_MAX_AGE_MS) {
      console.warn(
        `[setup-env] existing runId ${existing} is only ${(ageMs / 1000).toFixed(1)}s old, overwriting`
      );
    }
  }

  fs.writeFileSync(runIdPath, runId);
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
