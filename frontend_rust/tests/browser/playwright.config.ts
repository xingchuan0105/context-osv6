import { defineConfig } from '@playwright/test';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const frontendRustRoot = join(here, '..', '..');

// 端口：web-server 3200（SSR + hydrate 产物），fixture SSE 3201
const WEB_PORT = Number(process.env.POC_WEB_PORT || 3200);
const FIXTURE_PORT = Number(process.env.POC_FIXTURE_PORT || 3201);
const WEB_BASE = `http://127.0.0.1:${WEB_PORT}`;
const FIXTURE_BASE = `http://127.0.0.1:${FIXTURE_PORT}`;

export default defineConfig({
  testDir: '.',
  testMatch: '**/*.spec.ts',
  testIgnore: ['chat-live-smoke.spec.ts', 'gate0-perf.spec.ts'],
  timeout: 120_000,
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
  webServer: [
    {
      command: `node fixture-sse-server.mjs`,
      cwd: here,
      url: `${FIXTURE_BASE}/admin/state`,
      reuseExistingServer: false,
      env: { ...process.env, FIXTURE_PORT: String(FIXTURE_PORT) },
      timeout: 15_000,
    },
    {
      // 前置条件：已执行 `cargo leptos build`（产出 target/debug/web-server
      // 与 target/site/pkg 的 wasm 包）。直接跑二进制避免在测试内重复编译。
      command: join(frontendRustRoot, 'target', 'debug', 'web-server'),
      cwd: frontendRustRoot,
      url: `${WEB_BASE}/healthz`,
      reuseExistingServer: false,
      env: {
        ...process.env,
        LEPTOS_SITE_ADDR: `127.0.0.1:${WEB_PORT}`,
        LEPTOS_SITE_ROOT: join(frontendRustRoot, 'target', 'site'),
        LEPTOS_HASH_FILES: 'true',
      },
      timeout: 30_000,
    },
  ],
});
