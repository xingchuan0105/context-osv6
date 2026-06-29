import { chromium, request } from "@playwright/test";
import { TEST_USER } from "./fixtures/test-user";
import { ensureTestUserAccount, refreshE2eAuthStorageState } from "./utils/api-helpers";

/**
 * setup-auth 职责：确保预置账号存在并授予 admin，再用 API 登录写入 storageState。
 *
 * 必须在 grant-admin-role 之后重新登录，JWT 才会带上 org admin 权限（admin claim）。
 */
export default async function setupAuth() {
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000";
  const reqCtx = await request.newContext({ baseURL });
  try {
    await ensureTestUserAccount(reqCtx);
    await refreshE2eAuthStorageState(reqCtx);
    console.log("[setup-auth] test user ready with admin JWT in storageState");
  } finally {
    await reqCtx.dispose();
  }
}
