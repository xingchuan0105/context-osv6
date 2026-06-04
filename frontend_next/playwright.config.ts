import { defineConfig, devices } from "@playwright/test";
import { readFileSync } from "fs";

// Load .env so env vars are available in globalSetup / specs
for (const envFile of [".env.local", ".env"]) {
  try {
    const content = readFileSync(envFile, "utf-8");
    for (const line of content.split("\n")) {
      const m = line.match(/^([A-Za-z_]\w*)=(.*)$/);
      if (m && process.env[m[1]] === undefined) {
        process.env[m[1]] = m[2].replace(/^["'](.*)["']$/, "$1");
      }
    }
  } catch { /* file missing, skip */ }
}

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

  webServer: [
    // 本地测试 auth-flow 等纯前端场景时，可通过 SKIP_BACKEND=1 跳过 Rust 后端启动
    ...(process.env.SKIP_BACKEND
      ? []
      : [
          {
            command: "cd ../avrag-rs && cargo run --bin avrag-api",
            url: "http://127.0.0.1:8080/health",
            timeout: 120_000,
            reuseExistingServer: !process.env.CI,
          },
        ]),
    {
      // 统一由 Playwright webServer 启动前端；CI用 build+start，本地用 dev
      command: process.env.CI ? "pnpm build && pnpm start" : "pnpm dev",
      url: "http://127.0.0.1:3000",
      timeout: 60_000,
      reuseExistingServer: !process.env.CI,
    },
  ],

  projects: [
    {
      name: "functional",
      // 只匹配 specs/ 根目录下的 .spec.ts（不进入子目录），排除 auth-flow
      testMatch: [/specs\/[^/]*\.spec\.ts/],
      testIgnore: [/auth-flow\.spec\.ts/],
      use: {
        baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
        locale: "zh-CN",
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "retain-on-failure",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "auth",
      testMatch: [/auth.*\.spec\.ts/],
      use: {
        baseURL: process.env.PLAYWRIGHT_BASE_URL || "http://127.0.0.1:3000",
        locale: "zh-CN",
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
      name: "cross-browser-firefox",
      testMatch: [/specs\/[^/]*\.spec\.ts/],
      testIgnore: [/auth-flow\.spec\.ts/],
      use: {
        browserName: "firefox",
        storageState: "playwright/.auth/user.json",
      },
    },
    {
      name: "cross-browser-webkit",
      testMatch: [/specs\/[^/]*\.spec\.ts/],
      testIgnore: [/auth-flow\.spec\.ts/],
      use: {
        browserName: "webkit",
        storageState: "playwright/.auth/user.json",
      },
    },
  ],
});
