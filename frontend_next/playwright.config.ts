import { defineConfig, devices } from "@playwright/test";
import { readFileSync } from "fs";

function webServerEnv(extra: Record<string, string> = {}): Record<string, string> {
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(process.env)) {
    if (value !== undefined) {
      env[key] = value;
    }
  }
  return { ...env, ...extra };
}

function loadDotEnv(path: string) {
  try {
    const content = readFileSync(path, "utf-8");
    for (const line of content.split("\n")) {
      const m = line.match(/^([A-Za-z_]\w*)=(.*)$/);
      if (m && process.env[m[1]] === undefined) {
        process.env[m[1]] = m[2].replace(/^["'](.*)["']$/, "$1");
      }
    }
  } catch {
    /* file missing, skip */
  }
}

// Load .env so env vars are available in globalSetup / specs
for (const envFile of [".env.local", ".env", "../avrag-rs/.env"]) {
  loadDotEnv(envFile);
}

// Billing E2E prerequisite: Next inlines NEXT_PUBLIC_* when dev/build starts (after globalSetup).
process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED =
  process.env.NEXT_PUBLIC_PRICING_REVAMP_ENABLED ?? "1";

// Local avrag-ci-pg uses test:test; dev .env often has avrag/avrag — override for webServer only.
const DEFAULT_LOCAL_E2E_DATABASE_URL = "postgres://test:test@127.0.0.1:5432/test";
const playwrightDatabaseUrl =
  process.env.E2E_DATABASE_URL ??
  (process.env.CI ? process.env.DATABASE_URL : DEFAULT_LOCAL_E2E_DATABASE_URL);
const backendServerEnv: Record<string, string> = {
  E2E_ENABLED: process.env.E2E_ENABLED ?? "true",
  ...(playwrightDatabaseUrl
    ? { DATABASE_URL: playwrightDatabaseUrl, POSTGRES_URL: playwrightDatabaseUrl }
    : {}),
};

// Billing/journey E2E need PRICING_REVAMP_ROLLOUT + NEXT_PUBLIC_PRICING_REVAMP_ENABLED on
// freshly started servers. Opt in to reuse via PLAYWRIGHT_REUSE_SERVER=1 (local only).
const reuseExistingServer =
  Boolean(process.env.CI) === false && process.env.PLAYWRIGHT_REUSE_SERVER === "1";

export default defineConfig({
  testDir: "./e2e",
  timeout: 90_000,
  fullyParallel: false,
  workers: 1,
  // globalSetup 对外为单入口（兼容 Playwright API：string 而非 array[]），
  // 内部在 global-setup.ts 中串行调用 setupEnv() + setupAuth()。
  // 拆分原因：环境准备失败与认证失败的责任清晰分离；运维能单独重跑某一步。
  globalSetup: "./e2e/global-setup.ts",
  reporter: "list",

  // 共享配置：所有 project 继承，project 可覆盖
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
    locale: "zh-CN",
  },

  webServer: [
    // 本地测试 auth-flow 等纯前端场景时，可通过 SKIP_BACKEND=1 跳过 Rust 后端启动
    // Billing E2E: set PRICING_REVAMP_ROLLOUT=100 on avrag-api so test users pass hash-bucket gate.
    ...(process.env.SKIP_BACKEND
      ? []
      : [
          {
            // Prefer locked graph; CI should prebuild so this is mostly link+start.
            command: "cd ../avrag-rs && cargo run --locked --bin avrag-api",
            url: "http://127.0.0.1:8080/health",
            // 300s: first-time compile on cold runners can exceed 120s even with cache misses.
            timeout: 300_000,
            reuseExistingServer,
            env: webServerEnv({
              PRICING_REVAMP_ROLLOUT: "100",
              ...backendServerEnv,
            }),
          },
          {
            command: "cd ../avrag-rs && cargo run --locked -p avrag-worker",
            url: "http://127.0.0.1:8081/health",
            timeout: 300_000,
            reuseExistingServer,
            env: webServerEnv(backendServerEnv),
          },
        ]),
    {
      // 统一由 Playwright webServer 启动前端；CI用 build+start，本地用 dev
      command: process.env.CI ? "pnpm build && pnpm start" : "pnpm dev",
      url: "http://127.0.0.1:3000",
      // 300s: `pnpm build` (Next.js production build) on a 2-core CI runner takes
      // 90–180s, far exceeding the prior 60s timeout — which made the webServer fail
      // to start and red-flagged every journey/billing spec (observed 2026-06-29).
      timeout: 300_000,
      reuseExistingServer,
      env: webServerEnv({
        NEXT_PUBLIC_PRICING_REVAMP_ENABLED: "1",
      }),
    },
  ],

  projects: [
    {
      name: "functional",
      // PR 级：只跑 smoke（快速验证 UI 渲染和基础交互）
      testMatch: [/specs\/smoke\/[^/]*\.spec\.ts/],
      testIgnore: [/auth.*\.spec\.ts/],
      use: {
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "auth",
      testMatch: [/specs\/smoke\/auth.*\.spec\.ts/],
      use: {
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: { cookies: [], origins: [] },
      },
    },
    {
      name: "visual-desktop",
      testMatch: [/visual\/.*\.spec\.ts/],
      use: {
        browserName: "chromium",
        viewport: { width: 1440, height: 900 },
        storageState: "playwright/.auth/user.json",
        trace: "off",
        screenshot: "off",
        video: "off",
      },
    },
    {
      name: "visual-mobile",
      testMatch: [/visual\/.*\.spec\.ts/],
      use: {
        ...devices["Pixel 5"],
        storageState: "playwright/.auth/user.json",
        trace: "off",
        screenshot: "off",
        video: "off",
      },
    },
    {
      name: "journey",
      testMatch: [/specs\/journey\/[^/]*\.spec\.ts/],
      use: {
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: "playwright/.auth/user.json",
      },
    },
    // Cross-browser: experimental, opt-in via RUN_CROSS_BROWSER=1
    ...(process.env.RUN_CROSS_BROWSER
      ? [
          {
            name: "cross-browser-firefox",
            testMatch: [/specs\/journey\/[^/]*\.spec\.ts/],
            testIgnore: [/auth-flow\.spec\.ts/],
            use: {
              browserName: "firefox" as const,
              storageState: "playwright/.auth/user.json",
            },
          },
          {
            name: "cross-browser-webkit",
            testMatch: [/specs\/journey\/[^/]*\.spec\.ts/],
            testIgnore: [/auth-flow\.spec\.ts/],
            use: {
              browserName: "webkit" as const,
              storageState: "playwright/.auth/user.json",
            },
          },
        ]
      : []),
    {
      name: "skills",
      testMatch: [/specs\/skills\/.*\.spec\.ts/],
      retries: 1,
      use: {
        trace: "on-first-retry",
        video: "on-first-retry",
        screenshot: "only-on-failure",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "billing",
      testMatch: [/specs\/billing\/(?!visual-regression).*\.spec\.ts/],
      use: {
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "billing-visual",
      testMatch: [/specs\/billing\/visual-regression\.spec\.ts/],
      use: {
        browserName: "chromium",
        viewport: { width: 1280, height: 800 },
        storageState: "playwright/.auth/user.json",
        trace: "off",
        screenshot: "off",
        video: "off",
      },
    },
  ],
});
