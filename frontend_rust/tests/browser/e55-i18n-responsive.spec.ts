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

test.describe('E5.5 i18n + responsive', () => {
  test('账户菜单语言切换：中文 → English → 中文', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoApp(page, '/chat');
    await expect(page.getByTestId('new-chat-button')).toHaveText('新对话');
    await expect(page.getByTestId('send-button')).toHaveText('发送');

    await page.getByTestId('dashboard-account-menu-trigger').click();
    await expect(page.getByTestId('dashboard-account-menu')).toBeVisible();
    await page.getByTestId('account-locale-toggle').click();
    await page.getByTestId('account-locale-en').click();

    await expect(page.getByTestId('new-chat-button')).toHaveText('New chat');
    await expect(page.getByTestId('send-button')).toHaveText('Send');
    await expect(page.getByTestId('scope-cap-rag')).toContainText('Knowledge base');

    const localeToggle = page.getByTestId('account-locale-toggle');
    if (!(await localeToggle.isVisible())) {
      await page.getByTestId('dashboard-account-menu-trigger').click();
    }
    await localeToggle.click();
    await page.getByTestId('account-locale-zh-CN').click();
    await expect(page.getByTestId('new-chat-button')).toHaveText('新对话');
    expect(errors).toEqual([]);
  });

  test('/en/pricing 全英文', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await page.goto('/en/pricing', { waitUntil: 'domcontentloaded' });
    await expect(page.getByTestId('pricing-page')).toBeVisible();
    await expect(page.getByTestId('pricing-page')).toContainText('Choose your plan');
    await expect(page.getByTestId('pricing-page')).toContainText('Monthly');
    await expect(page.getByTestId('pricing-page')).toContainText('Yearly');
    await expect(page.getByTestId('pricing-page')).toContainText('FAQ');
    await expect(page.getByTestId('pricing-page')).not.toContainText('套餐定价');
    await expect(page.getByTestId('pricing-page')).not.toContainText('按月');
    await expect(page.getByTestId('mkt-nav-enter-app')).toHaveText('Open app');
    expect(errors).toEqual([]);
  });

  test('移动视口 chat 抽屉', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    const errors = collectPageErrors(page);
    await gotoApp(page, '/chat');
    await expect(page.getByTestId('chat-canvas')).toBeVisible();
    await expect(page.getByTestId('chat-rail-toggle')).toBeVisible();
    await page.getByTestId('chat-rail-toggle').click();
    await expect(page.getByTestId('session-list')).toBeVisible();
    await expect(page.getByTestId('new-chat-button')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('移动视口 dashboard 旅程', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    const errors = collectPageErrors(page);
    await gotoApp(page, '/dashboard');
    await expect(page.getByTestId('dashboard-overview')).toBeVisible();
    await expect(page.getByTestId('create-workspace-btn')).toBeVisible();
    await expect(page.getByTestId('workspace-card').first()).toBeVisible({ timeout: 15_000 });
    await page.getByTestId('dash-tab-favorites').click();
    await expect(page.getByTestId('page-empty')).toBeVisible();
    expect(errors).toEqual([]);
  });
});
