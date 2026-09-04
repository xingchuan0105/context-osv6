import { defineConfig } from "@playwright/test";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const frontendRustRoot = join(here, "..", "..");

const WEB_PORT = Number(process.env.POC_WEB_PORT || 3200);
const FIXTURE_PORT = Number(process.env.POC_FIXTURE_PORT || 3201);
const WEB_BASE = `http://127.0.0.1:${WEB_PORT}`;
const FIXTURE_BASE = `http://127.0.0.1:${FIXTURE_PORT}`;

export default defineConfig({
  testDir: ".",
  testMatch: "gate0-perf.spec.ts",
  timeout: 900_000,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: WEB_BASE,
    browserName: "chromium",
    headless: true,
    viewport: { width: 1440, height: 900 },
    actionTimeout: 20_000,
    navigationTimeout: 30_000,
  },
  webServer: [
    {
      command: "node fixture-sse-server.mjs",
      cwd: here,
      url: `${FIXTURE_BASE}/admin/state`,
      reuseExistingServer: false,
      env: { ...process.env, FIXTURE_PORT: String(FIXTURE_PORT) },
      timeout: 15_000,
    },
    {
      command: join(frontendRustRoot, "target", "debug", "web-server"),
      cwd: frontendRustRoot,
      url: `${WEB_BASE}/healthz`,
      reuseExistingServer: true,
      env: {
        ...process.env,
        LEPTOS_SITE_ADDR: `127.0.0.1:${WEB_PORT}`,
        LEPTOS_SITE_ROOT: join(frontendRustRoot, "target", "site"),
      },
      timeout: 30_000,
    },
  ],
});
