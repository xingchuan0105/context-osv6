import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const frontendRustRoot = join(here, '..', '..');

// 18080 在 avrag-api 默认 CORS 白名单内；3001/3200 不在。
const WEB_PORT = Number(process.env.POC_LIVE_WEB_PORT || 18080);
const WEB_BASE = `http://127.0.0.1:${WEB_PORT}`;

export default defineConfig({
  testDir: '.',
  testMatch: 'chat-live-smoke.spec.ts',
  timeout: 180_000,
  retries: 0,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL: WEB_BASE,
    browserName: 'chromium',
    headless: true,
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
  },
  webServer: {
    // 前置条件：已执行 `cargo leptos build`（产出 target/debug/web-server
    // 与 target/site/pkg）。直接跑二进制避免在测试内重复编译。
    command: join(frontendRustRoot, 'target', 'debug', 'web-server'),
    cwd: frontendRustRoot,
    url: `${WEB_BASE}/healthz`,
    reuseExistingServer: false,
    env: {
      ...process.env,
      LEPTOS_SITE_ADDR: `127.0.0.1:${WEB_PORT}`,
      LEPTOS_SITE_ROOT: join(frontendRustRoot, 'target', 'site'),
    },
    timeout: 30_000,
  },
});
