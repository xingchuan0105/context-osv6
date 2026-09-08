import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';
import { LIVE_API_BASE, obtainLiveJwt } from './live-auth';

const LIVE_ENABLED = process.env.LIVE_BACKEND === '1';

test.skip(!LIVE_ENABLED, 'set LIVE_BACKEND=1 to run against a live avrag-api');

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

async function gotoLive(page: Page, path: string, token?: string) {
  if (token) {
    await seedNextAuth(page, token);
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, LIVE_API_BASE);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('G5 live smoke（真实 avrag-api）', () => {
  test('J0 真实 Quick Chat 一轮', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/chat', token);
    await expect(page.getByTestId('chat-canvas')).toBeVisible();
    await page.getByTestId('composer-input').fill('只回答：pong');
    await page.getByTestId('send-button').click();
    await expect(page).toHaveURL(/\/chat\/[0-9a-fA-F-]{8,}$/, { timeout: 60_000 });
    await expect(page.getByTestId('live-answer')).not.toHaveText('', { timeout: 120_000 });
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 120_000 });
    expect(errors).toEqual([]);
  });

  test('J1 工作台总览可打开', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/dashboard', token);
    await expect(page.getByTestId('dashboard-overview')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('create-workspace-btn')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('J2 设置 Provider 面板', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/settings?tab=providers', token);
    await expect(page.getByTestId('settings-page')).toBeVisible();
    await expect(page.getByTestId('providers-panel')).toBeVisible({ timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('J3 定价页拉到真实套餐', async ({ page, request }) => {
    const errors = collectPageErrors(page, [/Failed to load resource/]);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/pricing', token);
    await expect(page.getByTestId('pricing-page')).toBeVisible();
    await expect(page.getByTestId('plans-grid')).toBeVisible({ timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('J4/J5 分享中心或全局分析可打开', async ({ page, request }) => {
    const errors = collectPageErrors(page, [/Failed to load resource/]);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/dashboard', token);
    await expect(page.getByTestId('dashboard-overview')).toBeVisible({ timeout: 15_000 });
    const card = page.getByTestId('workspace-card').first();
    if (await card.isVisible()) {
      await card.locator('a').first().click();
      await expect(page.getByTestId('workspace-workbench')).toBeVisible({ timeout: 15_000 });
      await page.getByTestId('goto-analyze').click();
      await expect(page).toHaveURL(/\/share/, { timeout: 15_000 });
    }
    await gotoLive(page, '/dashboard/analytics', token);
    await expect(page.getByTestId('global-analytics-page')).toBeVisible({ timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('J8 设置资料与安全 tab', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/settings?tab=profile', token);
    await expect(page.getByTestId('profile-panel')).toBeVisible({ timeout: 15_000 });
    await page.getByTestId('tab-security').click();
    await expect(page.getByTestId('security-panel')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('管理台巡检：有权限则概览，否则门禁', async ({ page, request }) => {
    const errors = collectPageErrors(page, [/Failed to load resource/]);
    const token = await obtainLiveJwt(request);
    await gotoLive(page, '/admin', token);
    const overview = page.getByTestId('admin-overview');
    const forbidden = page.getByTestId('admin-forbidden');
    const login = page.getByTestId('login-email');
    await expect(overview.or(forbidden).or(login)).toBeVisible({ timeout: 15_000 });
    expect(errors).toEqual([]);
  });
});
