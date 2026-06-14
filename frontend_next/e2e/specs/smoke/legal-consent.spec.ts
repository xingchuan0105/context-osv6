import { test, expect, type APIRequestContext } from "@playwright/test";

import {
  PUBLISHED_PRIVACY_VERSION,
  PUBLISHED_TERMS_VERSION,
} from "../../../lib/legal/versions";

/**
 * legal-consent.spec.ts — 覆盖 P1/P2 法律合规页与同意链路的端到端验证。
 *
 * 用例对应设计文档 §9.3：
 * - P0-IA-*  ：法律页可访问（无需登录）
 * - P0-CNT-* ：版本号与日期可见
 * - P0-UX-*  ：长文有 TOC 锚点
 * - P0-CON-1 ：未勾选不能注册
 * - P0-CON-4 ：stale version 后端拒绝
 * - §9.3.5   ：POST /api/auth/legal-acceptance re-acceptance 端点
 *
 * setup-auth 已准备 TEST_USER 凭据；本 spec 所有用例都从干净 storageState 出发。
 */

test.use({ storageState: { cookies: [], origins: [] } });

async function registerUser(
  request: APIRequestContext,
  email: string,
  fullName: string,
): Promise<string> {
  const resp = await request.post("/api/auth/register", {
    data: {
      email,
      password: "E2eTest123!",
      full_name: fullName,
      terms_version: PUBLISHED_TERMS_VERSION,
      privacy_version: PUBLISHED_PRIVACY_VERSION,
    },
  });
  expect(resp.status(), `registration for ${email} should succeed`).toBe(201);
  const body = await resp.json();
  const token = body.data?.token;
  expect(token, "registration should return JWT token").toBeTruthy();
  return token as string;
}

test.describe("Legal pages accessibility (P0-IA / P0-UX)", () => {
  test("/legal and sub-pages are public (no redirect to login)", async ({ page }) => {
    const publicPaths = [
      "/legal",
      "/legal/terms",
      "/legal/privacy",
      "/legal/licenses",
      "/legal/licenses/third-party",
      "/legal/licenses/project",
    ];
    for (const path of publicPaths) {
      const response = await page.goto(path);
      expect(response?.status(), `${path} should respond 200`).toBe(200);
      expect(page.url(), `${path} should not redirect to login`).not.toContain("/login");
    }
  });

  test("terms page exposes version date and at least 10 chapters", async ({ page }) => {
    await page.goto("/legal/terms");
    await expect(page.getByText(PUBLISHED_TERMS_VERSION).first()).toBeVisible();
    const headings = page.locator("article h2, .legal-document h2");
    const count = await headings.count();
    expect(count, "ToS should have ≥ 10 h2 chapters").toBeGreaterThanOrEqual(10);
  });

  test("privacy page explicitly states documents are NOT used for model training", async ({ page }) => {
    await page.goto("/legal/privacy");
    const body =
      (await page.locator("main, .legal-document, article").first().textContent()) ?? "";
    expect(body).toMatch(/不会|不会被/);
    expect(body).toMatch(/训练/);
  });

  test("long-form pages render a table of contents", async ({ page }) => {
    await page.goto("/legal/terms");
    const toc = page.locator(".legal-toc, nav[aria-label*='目录'], nav[aria-label*='TOC']");
    await expect(toc).toBeVisible();
  });
});

test.describe("Registration consent enforcement (P0-CON-1)", () => {
  test("register without consent stays on /register and shows error", async ({ page }) => {
    await page.goto("/register");
    const email = `noconsent-${Date.now()}@test.local`;
    await page.locator("#register-email").fill(email);
    await page.locator("#register-password").fill("E2eTest123!");
    await page.locator("#register-password-confirm").fill("E2eTest123!");
    await page.locator("#register-name").fill("No Consent");
    // 故意不勾选 consent，直接提交
    await page.getByRole("button", { name: /创建账号|Create account/i }).click();
    await page.waitForTimeout(500);
    expect(page.url()).toContain("/register");
    const alert = page.locator("[role='alert'], .app-notice-banner, .consent-error");
    await expect(alert.first()).toBeVisible();
  });

  test("register with consent navigates to /dashboard", async ({ page }) => {
    await page.goto("/register");
    const email = `e2e-${Date.now()}@test.local`;
    await page.locator("#register-email").fill(email);
    await page.locator("#register-password").fill("E2eTest123!");
    await page.locator("#register-password-confirm").fill("E2eTest123!");
    await page.locator("#register-name").fill("Consent E2E");
    await page.locator(".consent-input").check();
    await page.getByRole("button", { name: /创建账号|Create account/i }).click();
    await page.waitForURL(/\/dashboard$/, { timeout: 15_000 });
  });
});

test.describe("Backend legal-version validation (P0-CON-4)", () => {
  test("stale terms version is rejected with 400", async ({ request }) => {
    const response = await request.post("/api/auth/register", {
      data: {
        email: `stale-${Date.now()}@test.local`,
        password: "E2eTest123!",
        full_name: "Stale Version",
        terms_version: "2025-01-01",
        privacy_version: PUBLISHED_PRIVACY_VERSION,
      },
    });
    expect(response.status()).toBe(400);
    const body = await response.json();
    expect(String(body.error ?? body.code)).toMatch(
      /invalid_terms_version|consent_required/,
    );
  });

  test("missing terms_version is rejected with 400", async ({ request }) => {
    const response = await request.post("/api/auth/register", {
      data: {
        email: `missing-${Date.now()}@test.local`,
        password: "E2eTest123!",
        full_name: "Missing Terms",
        // terms_version 故意缺失
        privacy_version: PUBLISHED_PRIVACY_VERSION,
      },
    });
    expect(response.status()).toBe(400);
  });
});

test.describe("Legal status endpoint (P1-API-1)", () => {
  test("unauthenticated GET /api/auth/legal-status is 401", async ({ request }) => {
    const response = await request.get("/api/auth/legal-status");
    expect(response.status()).toBe(401);
  });

  test("authenticated GET /api/auth/legal-status returns published versions", async ({
    request,
  }) => {
    const token = await registerUser(
      request,
      `status-${Date.now()}@test.local`,
      "Legal Status",
    );
    const response = await request.get("/api/auth/legal-status", {
      headers: { Authorization: `Bearer ${token}` },
    });
    expect(response.status()).toBe(200);
    const body = await response.json();
    const status = body.data ?? body;
    expect(status.published_terms_version).toBe(PUBLISHED_TERMS_VERSION);
    expect(status.published_privacy_version).toBe(PUBLISHED_PRIVACY_VERSION);
    expect(typeof status.needs_re_acceptance).toBe("boolean");
  });
});

test.describe("Re-acceptance endpoint (§9.3.5)", () => {
  test("unauthenticated request to /api/auth/legal-acceptance is 401", async ({ request }) => {
    const response = await request.post("/api/auth/legal-acceptance", {
      data: {
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
        context: "payment",
      },
    });
    expect(response.status()).toBe(401);
  });

  test("authenticated re-acceptance with invalid context is 400", async ({ request }) => {
    const token = await registerUser(
      request,
      `reacc-${Date.now()}@test.local`,
      "Re-accept",
    );
    const resp = await request.post("/api/auth/legal-acceptance", {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
        context: "bogus",
      },
    });
    expect(resp.status()).toBe(400);
  });

  test("authenticated re-acceptance with payment context is 201", async ({ request }) => {
    const token = await registerUser(
      request,
      `pay-${Date.now()}@test.local`,
      "Payment Accept",
    );
    const resp = await request.post("/api/auth/legal-acceptance", {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
        context: "payment",
      },
    });
    expect(resp.status()).toBe(201);
  });

  test("authenticated re-acceptance with re_acceptance context is 201", async ({
    request,
  }) => {
    const token = await registerUser(
      request,
      `reacc-ctx-${Date.now()}@test.local`,
      "Re-accept Ctx",
    );
    const resp = await request.post("/api/auth/legal-acceptance", {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        terms_version: PUBLISHED_TERMS_VERSION,
        privacy_version: PUBLISHED_PRIVACY_VERSION,
        context: "re_acceptance",
      },
    });
    expect(resp.status()).toBe(201);
  });
});

test.describe("Re-acceptance gate UI (P1-FE-2)", () => {
  test("dashboard shows re-acceptance panel when legal-status requires it", async ({
    page,
  }) => {
    const token = "e2e-mock-token";
    const user = {
      id: "e2e-user",
      email: "e2e-mock@example.com",
      full_name: "E2E Mock",
    };

    await page.addInitScript(({ authToken, authUser }) => {
      window.localStorage.setItem(
        "avrag.auth.v1",
        JSON.stringify({ token: authToken, user: authUser }),
      );
    }, { authToken: token, authUser: user });

    await page.route("**/api/auth/me", (route) =>
      route.fulfill({
        json: {
          success: true,
          data: { user },
        },
      }),
    );

    await page.route("**/api/auth/legal-status", (route) =>
      route.fulfill({
        json: {
          success: true,
          data: {
            needs_re_acceptance: true,
            published_terms_version: PUBLISHED_TERMS_VERSION,
            published_privacy_version: PUBLISHED_PRIVACY_VERSION,
          },
        },
      }),
    );

    await page.goto("/dashboard");
    await expect(page.getByText(/协议已更新|Terms updated/i)).toBeVisible();
    await expect(page.locator(".consent-input")).toBeVisible();
    await expect(
      page.getByRole("button", { name: /确认并继续|Confirm and continue/i }),
    ).toBeVisible();
  });
});

test.describe("Payment consent UI (P1-FE-3)", () => {
  test("pricing upgrade without consent shows error", async ({ page }) => {
    await page.route("**/api/v1/billing/checkout-session", (route) =>
      route.fulfill({
        status: 400,
        json: { ok: false, error: "consent_required" },
      }),
    );
    await page.goto("/pricing");
    await page.getByRole("button", { name: /升级 Plus|Upgrade Plus/i }).first().click();
    const alert = page.locator("[role='alert'], .app-notice-banner, .consent-error");
    await expect(alert.first()).toBeVisible({ timeout: 5_000 });
  });
});
