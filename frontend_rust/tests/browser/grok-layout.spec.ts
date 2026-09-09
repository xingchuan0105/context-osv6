import { test, expect } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const fixture = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

test.beforeEach(async ({ page, request }) => {
  await request.post(`${fixture}/admin/reset`);
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, fixture);
});

test('desktop navigation collapse persists without replacing the chat draft', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto('/chat', { waitUntil: 'networkidle' });
  await page.getByTestId('composer-input').fill('Keep the current draft');
  await page.getByTestId('navigation-collapse').click();
  await expect(page.getByTestId('session-list')).toHaveCSS('width', '60px');
  const shortcuts = page.locator('.app-navigation-shortcuts');
  await expect(shortcuts.getByRole('link').nth(0)).toHaveText('聊天');
  await expect(shortcuts.getByRole('link').nth(1)).toHaveText('工作区');
  await expect(shortcuts.getByRole('link').nth(1)).toHaveAttribute('href', '/dashboard');
  await expect(shortcuts.getByRole('link').nth(0)).toHaveAttribute('aria-current', 'page');
  await expect(page.getByTestId('composer-input')).toHaveValue('Keep the current draft');
  await page.reload({ waitUntil: 'networkidle' });
  await expect(page.getByTestId('session-list')).toHaveCSS('width', '60px');
  await page.getByTestId('navigation-collapse').click();
  await expect(page.getByTestId('session-list')).toHaveCSS('width', '248px');
  await expect(page.getByTestId('dashboard-account-menu-trigger')).toHaveCount(1);
  await expect(page.getByTestId('app-topbar-share-menu')).toHaveCount(0);
  await expect(page.getByTestId('all-workspaces-link')).toHaveText('工作区');
  await expect(page.locator('.chat-workspace-link')).toHaveCount(0);
});

test('mobile drawer traps focus, dismisses with Escape, and keeps the draft', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/chat', { waitUntil: 'networkidle' });
  await page.getByTestId('composer-input').fill('手机草稿');
  await page.getByTestId('chat-rail-toggle').click();
  await expect(page.getByTestId('session-list')).toBeVisible();
  await expect(page.getByTestId('navigation-collapse')).toBeFocused();
  await page.getByTestId('app-topbar-brand').focus();
  await page.keyboard.press('Shift+Tab');
  await expect(page.getByTestId('dashboard-account-menu-trigger')).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('session-list')).toBeHidden();
  await expect(page.getByTestId('chat-rail-toggle')).toBeFocused();
  await expect(page.getByTestId('composer-input')).toHaveValue('手机草稿');
});

for (const width of [390, 768, 1280, 1440]) {
  test(`composer remains a single toolbar at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 720 });
    await page.goto('/chat', { waitUntil: 'networkidle' });
    const add = await page.getByTestId('turn-attachment-add').boundingBox();
    const search = await page.getByTestId('scope-cap-search').boundingBox();
    const send = await page.getByTestId('send-button').boundingBox();
    expect(Math.abs(add!.y - send!.y)).toBeLessThanOrEqual(4);
    expect(Math.abs(search!.y - send!.y)).toBeLessThanOrEqual(4);
    expect(add!.width).toBe(44);
    expect(send!.y + send!.height).toBeLessThanOrEqual(720);
    const hero = await page.getByTestId('chat-hero').boundingBox();
    const composer = await page.getByTestId('chat-composer').boundingBox();
    expect(composer!.y - hero!.y - hero!.height).toBeLessThan(60);
    await expect(page.getByTestId('scope-cap-rag')).toHaveCount(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
  });
}
