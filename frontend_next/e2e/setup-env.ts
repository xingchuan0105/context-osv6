import { request } from "@playwright/test";
import { resetTestUserData, waitForBackendHealth } from "./utils/api-helpers";

/**
 * setup-env 职责：纯环境准备，无浏览器参与。
 *   1. 生成 runId 并持久化到磁盘（供所有 spec 和后续 setup-auth 复用）
 *   2. 重置预置账号数据（清空上一 run 的残留）
 *   3. （billing）依赖 webServer 注入的 pricing revamp 环境变量
 *
 * 设计理由：这一步完全不需要浏览器。如果 runId 生成或 reset 失败，
 * 失败原因明确归为"环境/后端"问题，而不是"登录流程"问题。
 *
 * Pricing revamp 开关由 playwright.config.ts webServer.env 在进程启动时注入：
 *   - avrag-api: PRICING_REVAMP_ROLLOUT=100（E2E 用户通过 hash 桶）
 *   - Next.js: NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1（须在 dev/build 前设置，globalSetup 太晚）
 * 本地默认不复用已有 webServer（见 PLAYWRIGHT_REUSE_SERVER）；避免 rollout 环境丢失。
 */
const RUN_ID_MAX_AGE_MS = 10 * 60 * 1000; // 10 分钟

async function ensureEnvLocalEntry(key: string, value: string) {
  const fs = await import("fs");
  const envLocalPath = ".env.local";
  const lines = fs.existsSync(envLocalPath)
    ? fs.readFileSync(envLocalPath, "utf-8").split("\n")
    : [];
  const filtered = lines.filter((line) => line.trim() && !line.startsWith(`${key}=`));
  filtered.push(`${key}=${value}`);
  fs.writeFileSync(envLocalPath, `${filtered.join("\n")}\n`);
}

export default async function setupEnv() {
  // globalSetup runs before Playwright webServer; persist flag for `pnpm dev` / `next build`.
  await ensureEnvLocalEntry(
    "NEXT_PUBLIC_PRICING_REVAMP_ENABLED",
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED ?? "1",
  );
  process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED =
    process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED ?? "1";

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
    await waitForBackendHealth();
    await resetTestUserData(reqCtx);
    console.log("[setup-env] reset-user-data succeeded");
  } catch (e) {
    console.error("[setup-env] reset-user-data failed:", e);
    throw e;
  } finally {
    await reqCtx.dispose();
  }
}
