import { type APIRequestContext } from "@playwright/test";
import { readFileSync } from "fs";
import {
  PUBLISHED_PRIVACY_VERSION,
  PUBLISHED_TERMS_VERSION,
} from "../../lib/legal/versions";
import { TEST_USER, COLLAB_USER } from "../fixtures/test-user";

const AUTH_STORAGE_STATE_PATH = "playwright/.auth/user.json";
const AUTH_STORAGE_KEY = "avrag.auth.v1";
const AUTH_PERSISTED_COOKIE = "avrag.auth.persisted";
const AUTH_SESSION_COOKIE = "avrag.auth.session";

type AuthUser = {
  id: string;
  email: string;
  full_name: string;
};

type AuthPayload = {
  token: string;
  user: AuthUser;
};

type StorageState = {
  cookies: Array<{
    name: string;
    value: string;
    domain: string;
    path: string;
    expires: number;
    httpOnly: boolean;
    secure: boolean;
    sameSite: "Lax" | "Strict" | "None";
  }>;
  origins: Array<{
    origin: string;
    localStorage: Array<{ name: string; value: string }>;
  }>;
};

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
 * 将 API 登录结果写回 Playwright storageState。
 * spec 的 beforeAll 在 reset-user-data（删除用户）后必须调用，否则页面仍携带失效 JWT。
 */
export function writeAuthStorageState(payload: AuthPayload) {
  const baseURL = process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000";
  const origin = new URL(baseURL).origin;
  const domain = new URL(baseURL).hostname;
  const expires = Date.now() / 1000 + 60 * 60 * 24 * 365;
  const persistedCookieValue = encodeURIComponent(
    JSON.stringify({ token: payload.token, user: payload.user }),
  );
  const localStorageValue = JSON.stringify({
    token: payload.token,
    user: payload.user,
  });

  let state: StorageState = { cookies: [], origins: [] };
  try {
    state = JSON.parse(readFileSync(AUTH_STORAGE_STATE_PATH, "utf-8")) as StorageState;
  } catch {
    // First run: bootstrap a minimal storageState shell.
  }

  const upsertCookie = (
    name: string,
    value: string,
  ) => {
    const existing = state.cookies.find((cookie) => cookie.name === name);
    const cookie = {
      name,
      value,
      domain,
      path: "/",
      expires,
      httpOnly: false,
      secure: false,
      sameSite: "Lax" as const,
    };
    if (existing) {
      Object.assign(existing, cookie);
    } else {
      state.cookies.push(cookie);
    }
  };

  upsertCookie(AUTH_PERSISTED_COOKIE, persistedCookieValue);
  upsertCookie(AUTH_SESSION_COOKIE, "1");

  let originEntry = state.origins.find((entry) => entry.origin === origin);
  if (!originEntry) {
    originEntry = { origin, localStorage: [] };
    state.origins.push(originEntry);
  }

  const authItem = originEntry.localStorage.find((item) => item.name === AUTH_STORAGE_KEY);
  if (authItem) {
    authItem.value = localStorageValue;
  } else {
    originEntry.localStorage.push({ name: AUTH_STORAGE_KEY, value: localStorageValue });
  }

  const fs = require("fs") as typeof import("fs");
  fs.mkdirSync("playwright/.auth", { recursive: true });
  fs.writeFileSync(AUTH_STORAGE_STATE_PATH, JSON.stringify(state, null, 2));
}

async function loginUserViaAPI(
  request: APIRequestContext,
  email: string,
  password: string,
): Promise<AuthPayload> {
  const loginResp = await request.post("/api/auth/login", {
    data: { email, password },
    timeout: 30_000,
  });
  if (!loginResp.ok()) {
    throw new Error(`login failed: ${loginResp.status()} ${await loginResp.text()}`);
  }

  const body = (await loginResp.json()) as {
    success?: boolean;
    data?: AuthPayload | null;
    error?: string | null;
  };
  if (!body.success || !body.data?.token || !body.data.user) {
    throw new Error(`login response invalid: ${JSON.stringify(body)}`);
  }
  return body.data;
}

async function loginTestUserViaAPI(request: APIRequestContext): Promise<AuthPayload> {
  return loginUserViaAPI(request, TEST_USER.email, TEST_USER.password);
}

/** API login + legal acceptance for secondary E2E users (e.g. invite collaborator). */
export async function loginAndPrepareUserSession(
  request: APIRequestContext,
  email: string,
  password: string,
): Promise<AuthPayload> {
  const payload = await loginUserViaAPI(request, email, password);
  await ensureLegalAcceptance(request, payload.token);
  return payload;
}

/** Seed a fresh browser context/page with JWT + persisted auth cookies. */
export async function seedBrowserPageAuth(
  page: import("@playwright/test").Page,
  payload: AuthPayload,
) {
  const localStorageValue = JSON.stringify({ token: payload.token, user: payload.user });
  const persistedCookieValue = encodeURIComponent(localStorageValue);
  await page.goto("/login");
  await page.evaluate(
    ({ storageKey, storageValue, persistedValue }) => {
      localStorage.setItem(storageKey, storageValue);
      const maxAge = 60 * 60 * 24 * 365;
      document.cookie = `avrag.auth.persisted=${persistedValue}; Path=/; SameSite=Lax; Max-Age=${maxAge}`;
      document.cookie = `avrag.auth.session=1; Path=/; SameSite=Lax; Max-Age=${maxAge}`;
    },
    {
      storageKey: AUTH_STORAGE_KEY,
      storageValue: localStorageValue,
      persistedValue: persistedCookieValue,
    },
  );
}

async function ensureLegalAcceptance(request: APIRequestContext, token: string) {
  const resp = await request.post("/api/auth/legal-acceptance", {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      terms_version: PUBLISHED_TERMS_VERSION,
      privacy_version: PUBLISHED_PRIVACY_VERSION,
      context: "re_acceptance",
    },
    timeout: 30_000,
  });
  if (!resp.ok()) {
    throw new Error(`legal-acceptance failed: ${resp.status()} ${await resp.text()}`);
  }
}

/** 用 API 重新登录并刷新 storageState（globalSetup 之后、spec beforeAll 内使用）。 */
export async function refreshE2eAuthStorageState(request: APIRequestContext) {
  const payload = await loginTestUserViaAPI(request);
  await ensureLegalAcceptance(request, payload.token);
  writeAuthStorageState(payload);
}

/**
 * 重置预置账号数据并刷新浏览器 storageState。
 * reset-user-data 会级联删除用户；必须在 spec beforeAll 中用它替代裸 resetTestUserData。
 */
export async function resetAndPrepareTestUser(request: APIRequestContext) {
  await resetTestUserData(request);
  await ensureTestUserAccount(request);
  await refreshE2eAuthStorageState(request);
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Poll avrag-api /health before globalSetup calls E2E reset endpoints.
 * Playwright marks Next.js ready before Rust backends finish cold starts on repeat runs.
 */
export async function waitForBackendHealth(timeoutMs = 90_000) {
  const healthUrl = `${process.env.E2E_API_HEALTH_URL ?? "http://127.0.0.1:8080/health"}`;
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(healthUrl);
      if (response.ok) {
        return;
      }
    } catch {
      // Backend still starting.
    }

    await sleep(500);
  }

  throw new Error(`backend health check failed: ${healthUrl}`);
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

  const maxAttempts = 4;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const resp = await request.post("/api/e2e/reset-user-data", {
      headers: { "X-E2E-Secret": secret },
      data: { email: TEST_USER.email },
      timeout: 30_000,
    });

    if (resp.ok()) {
      return;
    }

    const body = await resp.text();
    const retryable = resp.status() >= 500 || resp.status() === 429;
    if (retryable && attempt < maxAttempts) {
      await sleep(500 * attempt);
      continue;
    }

    throw new Error(`reset-user-data failed: ${resp.status()} ${body}`);
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
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
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
 * 将协作者账号放入 owner 所在 org，供 invite-accept E2E 使用。
 * 须在 owner 账号存在（reset + ensureTestUserAccount）之后调用。
 */
export async function ensureE2eOrgMember(
  request: APIRequestContext,
  ownerEmail: string = TEST_USER.email,
  member = COLLAB_USER,
) {
  const secret = process.env.E2E_RESET_SECRET;
  if (!secret) {
    throw new Error("E2E_RESET_SECRET is required. Set it in .env or environment.");
  }
  const resp = await request.post("/api/e2e/ensure-org-member", {
    headers: { "X-E2E-Secret": secret },
    data: {
      owner_email: ownerEmail,
      member_email: member.email,
      password: member.password,
      full_name: member.fullName,
    },
    timeout: 30_000,
  });
  if (!resp.ok()) {
    throw new Error(`ensure-org-member failed: ${resp.status()} ${await resp.text()}`);
  }
}

export async function listNotebookMembers(request: APIRequestContext, notebookId: string) {
  const resp = await request.get(`/api/v1/workspaces/${notebookId}/members`, {
    headers: authHeaders(),
  });
  if (!resp.ok()) {
    throw new Error(`list members failed: ${resp.status()} ${await resp.text()}`);
  }
  return resp.json() as Promise<{
    members: Array<{
      member_id: string;
      email: string;
      status: string;
      role: string;
    }>;
  }>;
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
  const resp = await request.post("/api/v1/workspaces", {
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
  await request.delete(`/api/v1/workspaces/${notebookId}`, {
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
