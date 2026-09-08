import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;
const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '../../../docs/engineering/_reports/2026-09-08-g5/rust');

async function gotoApp(page: Page, path: string, auth = true) {
  if (auth) {
    await seedNextAuth(page, 'poc-test-token');
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('G5 感知走查截图（Rust fixture）', () => {
  test.beforeAll(() => {
    mkdirSync(outDir, { recursive: true });
  });

  test('八个关键页面截图', async ({ page }) => {
    const shots: Array<[string, string, string, boolean]> = [
      ['/chat', 'chat-canvas', 'chat.png', true],
      ['/dashboard', 'dashboard-overview', 'dashboard.png', true],
      ['/dashboard/ws-materials', 'workspace-workbench', 'workbench.png', true],
      ['/settings', 'settings-page', 'settings.png', true],
      ['/pricing', 'pricing-page', 'pricing.png', true],
      ['/login', 'login-email', 'login.png', false],
      ['/dashboard/ws-materials/share', 'workspace-share-page', 'share.png', true],
      ['/admin', 'admin-overview', 'admin.png', true],
    ];
    for (const [path, testId, file, auth] of shots) {
      await gotoApp(page, path, auth);
      await expect(page.getByTestId(testId)).toBeVisible({ timeout: 15_000 });
      await page.screenshot({ path: join(outDir, file), fullPage: true });
    }
  });
});
