import { type APIRequestContext } from "@playwright/test";
import { readFileSync } from "fs";
import { TEST_USER } from "../fixtures/test-user";

const AUTH_STORAGE_STATE_PATH = "playwright/.auth/user.json";
const AUTH_STORAGE_KEY = "avrag.auth.v1";
const AUTH_PERSISTED_COOKIE = "avrag.auth.persisted";

/**
 * 从 globalSetup 生成的 storageState 读取 JWT，供 APIRequestContext 使用。
 * page.request / request fixture 只自动携带 cookie，后端 /api/v1/* 需要 Bearer token。
 */
export function readAuthToken(): string {
  const state = JSON.parse(readFileSync(AUTH_STORAGE_STATE_PATH, "utf-8")) as {
    cookies?: Array<{ name: string; value: string }>;
    origins?: Array<{ localStorage?: Array<{ name: string; value: string }> }>;
  };

  for (const origin of state.origins ?? []) {
    for (const item of origin.localStorage ?? []) {
      if (item.name === AUTH_STORAGE_KEY) {
        const parsed = JSON.parse(item.value) as { token?: string };
        if (parsed.token) return parsed.token;
      }
    }
  }

  const cookie = state.cookies?.find((c) => c.name === AUTH_PERSISTED_COOKIE);
  if (cookie) {
    const parsed = JSON.parse(decodeURIComponent(cookie.value)) as { token?: string };
    if (parsed.token) return parsed.token;
  }

  throw new Error(`No auth token in ${AUTH_STORAGE_STATE_PATH}. Re-run globalSetup (setup-auth).`);
}

export function authHeaders(): Record<string, string> {
  return { Authorization: `Bearer ${readAuthToken()}` };
}

/**
 * 重置预置测试账号的所有数据。
 * 主要用于 globalSetup/teardown，也可在 spec 的 beforeAll 中使用。
 */
export async function resetTestUserData(request: APIRequestContext) {
  const secret = process.env.E2E_RESET_SECRET;
  if (!secret) {
    throw new Error("E2E_RESET_SECRET is required for globalSetup. Set it in .env or environment.");
  }
  const resp = await request.post("/api/e2e/reset-user-data", {
    headers: { "X-E2E-Secret": secret },
    data: { email: TEST_USER.email },
    timeout: 30_000,
  });
  if (!resp.ok()) {
    throw new Error(`reset-user-data failed: ${resp.status()} ${await resp.text()}`);
  }
}

/**
 * 确保预置 E2E 账号存在（必要时 API 注册），并授予 super_admin。
 * 须在 reset-user-data 之后、浏览器登录之前调用。
 */
export async function ensureTestUserAccount(request: APIRequestContext) {
  const loginResp = await request.post("/api/auth/login", {
    data: { email: TEST_USER.email, password: TEST_USER.password },
    timeout: 30_000,
  });

  if (!loginResp.ok()) {
    const registerResp = await request.post("/api/auth/register", {
      data: {
        email: TEST_USER.email,
        password: TEST_USER.password,
        full_name: TEST_USER.fullName,
      },
      timeout: 30_000,
    });
    if (!registerResp.ok()) {
      throw new Error(`register failed: ${registerResp.status()} ${await registerResp.text()}`);
    }
  }

  await grantTestUserAdminRole(request);
}

/**
 * 为预置 E2E 测试账号授予 super_admin 角色，供 admin-navigation 等后台 smoke 使用。
 */
export async function grantTestUserAdminRole(request: APIRequestContext) {
  const secret = process.env.E2E_RESET_SECRET;
  if (!secret) {
    throw new Error("E2E_RESET_SECRET is required for globalSetup. Set it in .env or environment.");
  }
  const resp = await request.post("/api/e2e/grant-admin-role", {
    headers: { "X-E2E-Secret": secret },
    data: { email: TEST_USER.email },
    timeout: 30_000,
  });
  if (!resp.ok()) {
    throw new Error(`grant-admin-role failed: ${resp.status()} ${await resp.text()}`);
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
export async function createNotebookViaAPI(
  request: APIRequestContext,
  name: string,
  description = "",
) {
  const resp = await request.post("/api/v1/notebooks", {
    headers: authHeaders(),
    data: { name, description },
  });
  if (!resp.ok()) {
    throw new Error(`create notebook failed: ${resp.status()} ${await resp.text()}`);
  }
  return resp.json() as Promise<{ notebook: { id: string } }>;
}

export async function deleteNotebookViaAPI(
  request: APIRequestContext,
  notebookId: string,
) {
  if (!notebookId) return;
  await request.delete(`/api/v1/notebooks/${notebookId}`, {
    headers: authHeaders(),
  });
}

/**
 * 轮询文档 ingestion 状态直到 completed/ready（与 Rust wait_for_ingestion 对齐）。
 */
export async function waitForDocumentReady(
  request: APIRequestContext,
  documentId: string,
  timeoutMs = 120_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  const terminal = new Set(["completed", "ready", "Completed", "Ready"]);

  while (Date.now() < deadline) {
    const resp = await request.get(`/api/v1/documents/${documentId}/status`, {
      headers: authHeaders(),
    });
    if (resp.ok()) {
      const body = (await resp.json()) as { status?: string };
      if (body.status && terminal.has(body.status)) {
        return;
      }
    }
    await new Promise((r) => setTimeout(r, 2000));
  }

  throw new Error(`document ${documentId} not ready within ${timeoutMs}ms`);
}
