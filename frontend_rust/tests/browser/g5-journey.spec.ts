import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

function collectPageErrors(page: Page, allow: RegExp[] = []) {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(`pageerror: ${error.message}`));
  page.on('console', (message) => {
    if (message.type() !== 'error') return;
    const text = message.text();
    if (/favicon/.test(text)) return;
    if (allow.some((pattern) => pattern.test(text))) return;
    errors.push(`console.error: ${text}`);
  });
  return errors;
}

async function gotoApp(page: Page, path: string, token = 'poc-test-token', apiBase = FIXTURE_BASE) {
  if (token) {
    await seedNextAuth(page, token);
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, apiBase);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('G5 PRODUCT_IA §1 旅程（fixture）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('J0 无库直聊含会话文件', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/chat', 'poc-test-token', `${FIXTURE_BASE}/case/files`);
    await expect(page.getByTestId('chat-canvas')).toBeVisible();
    await expect(page.getByTestId('chat-hero')).toBeVisible();
    const fileInput = page.getByTestId('session-file-input');
    await expect(fileInput).toHaveAttribute('data-listening', 'true', { timeout: 15_000 });
    await fileInput.setInputFiles({
      name: 'notes.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('hello'),
    });
    await fileInput.dispatchEvent('change');
    await expect(page).toHaveURL(/\/chat\/sess-file-1$/, { timeout: 15_000 });
    await expect(page.getByTestId('session-file-status')).toHaveText('就绪', { timeout: 15_000 });
    await page.getByTestId('composer-input').fill('文件已就绪');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const state = await request.get(`${FIXTURE_BASE}/admin/state`).then((r) => r.json());
    expect(state.lastChatBody?.capabilities).toEqual(['rag']);
    expect(errors).toEqual([]);
  });

  test('J1 建库并进入工作台', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/dashboard');
    await expect(page.getByTestId('workspace-card').filter({ hasText: '材料研发' })).toBeVisible({
      timeout: 15_000,
    });
    await page.getByTestId('create-workspace-btn').click();
    await expect(page.getByRole('dialog', { name: '新建工作区' })).toBeVisible();
    await page.getByTestId('new-workspace-name').fill('G5 建库');
    await page.getByTestId('submit-create-workspace').click();
    await expect(page).toHaveURL(/\/dashboard\/ws-/);
    await expect(page.getByTestId('workspace-workbench')).toBeVisible();
    await expect(page.getByTestId('workspace-side-rail')).toBeHidden();
    await page.getByTestId('workspace-open-sources').click();
    await expect(page.getByTestId('workspace-side-rail')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('J2 BYOK 闭环', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/login?next=/settings', '');
    await page.getByTestId('login-email').fill('dave@example.com');
    await page.getByTestId('login-password').fill('pass123456');
    await page.getByTestId('login-submit').click();
    await expect(page).toHaveURL(/\/settings$/);
    await expect(page.getByTestId('providers-panel')).toBeVisible();
    await page.getByTestId('input-quick_chat').fill('sk-g5-byok');
    await page.getByTestId('save-quick_chat').click();
    await expect(page.getByTestId('status-quick_chat')).toContainText('已配置');
    await page.getByTestId('revoke-quick_chat').click();
    await expect(page.getByTestId('input-quick_chat')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('J3 充值/升级', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/pricing');
    await expect(page.getByTestId('plan-free')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('plan-pro')).toBeVisible();
    await page.getByTestId('pricing-agree').check();
    await page.getByTestId('btn-start-topup').click();
    await expect(page.getByTestId('checkout-redirect-box')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('pay-link')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('J4 分享开链 + 访客页', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await gotoApp(page, '/dashboard/ws-materials/share');
    await expect(page.getByTestId('workspace-share-page')).toBeVisible();
    await expect(page.getByTestId('share-active-box')).toBeVisible();
    await page.goto('/shared/kb/tok-valid-123', { waitUntil: 'domcontentloaded' });
    await expect(page.getByTestId('shared-kb-page')).toBeVisible();
    await expect(page.getByTestId('chat-canvas')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('J5 分享效果查看', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/dashboard/ws-materials/share/analytics');
    await expect(page.getByTestId('share-analytics-page')).toBeVisible();
    await gotoApp(page, '/dashboard/analytics');
    await expect(page.getByTestId('global-analytics-page')).toBeVisible();
    await expect(page.getByTestId('analytics-views')).toBeVisible({ timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('J8 设置与安全', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/settings?tab=profile');
    await expect(page.getByTestId('profile-panel')).toBeVisible();
    await page.getByTestId('tab-security').click();
    await expect(page.getByTestId('security-panel')).toBeVisible();
    await page.getByTestId('settings-logout').click();
    await expect(page).toHaveURL(/\/login$/);
    expect(errors).toEqual([]);
  });

  test('管理台巡检', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/admin');
    await expect(page.getByTestId('admin-overview')).toBeVisible();
    await expect(page.getByTestId('admin-overview-grid')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-entry-accounts')).toBeVisible();
    await expect(page.getByTestId('admin-entry-audit-logs')).toBeVisible();
    expect(errors).toEqual([]);
  });
});
