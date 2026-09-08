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

async function gotoApp(page: Page, path: string) {
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('E5.4 业务页真数据与交互件', () => {
  test('用量页绑定 API 并绘制趋势图', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/settings/usage');
    await expect(page.getByTestId('usage-page')).toBeVisible();
    await expect(page.getByTestId('usage-5h')).toHaveText('120', { timeout: 15_000 });
    await expect(page.getByTestId('usage-7d')).toHaveText('800');
    await expect(page.getByTestId('usage-trend-chart')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('工作区总览 tabs / 搜索弹窗 / 三态', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/dashboard');
    await expect(page.getByTestId('workspace-card')).toBeVisible({ timeout: 15_000 });
    await page.getByTestId('dash-tab-favorites').click();
    await expect(page.getByTestId('page-empty')).toBeVisible();
    await page.getByTestId('dash-tab-all').click();
    await page.getByTestId('dash-search-open').click();
    await expect(page.getByTestId('dashboard-search-dialog')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('工作台上传弹窗三 tab', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/dashboard/ws-materials');
    await expect(page.getByTestId('workspace-workbench')).toBeVisible();
    await page.getByTestId('open-upload').click();
    await expect(page.getByTestId('upload-dialog')).toBeVisible();
    await page.getByTestId('upload-tab-url').click();
    await expect(page.getByTestId('workspace-url-input')).toBeVisible();
    await page.getByTestId('upload-tab-paste').click();
    await expect(page.getByTestId('workspace-paste-body')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('设置页 billing / security / profile 面板', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/settings?tab=profile');
    await expect(page.getByTestId('profile-panel')).toBeVisible();
    await page.getByTestId('tab-billing').click();
    await expect(page.getByTestId('billing-panel')).toBeVisible();
    await page.getByTestId('tab-security').click();
    await expect(page.getByTestId('security-panel')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('定价页绑定 API 套餐且含 FAQ 与同意勾选', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/pricing');
    await expect(page.getByTestId('plan-free')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('plan-pro')).toBeVisible();
    await expect(page.getByTestId('pricing-faq')).toBeVisible();
    await expect(page.getByTestId('pricing-agree')).toBeVisible();
    expect(errors).toEqual([]);
  });
});
