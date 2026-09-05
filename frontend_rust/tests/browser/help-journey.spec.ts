import { test, expect, type Page } from '@playwright/test';

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

test.describe('应用内帮助中心（E3.5）', () => {
  test('帮助中心入口卡渲染并覆盖已挂载规范路由', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-page')).toBeVisible();
    await expect(page.getByTestId('help-card-write')).toBeVisible();
    await expect(page.getByTestId('help-card-providers')).toBeVisible();
    await expect(page.getByTestId('help-card-pricing')).toBeVisible();
    await expect(page.getByTestId('help-card-desktop')).toBeVisible();
    await expect(page.getByTestId('help-card-dashboard')).toBeVisible();

    expect(errors).toEqual([]);
  });

  test('帮助中心与长文指引互链可达', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help', { waitUntil: 'domcontentloaded' });

    await page.getByTestId('help-card-write').getByRole('link').click();
    await expect(page.getByTestId('help-write-page')).toBeVisible();
    await expect(page.getByTestId('help-write-page')).toContainText('长文写作');
    await expect(page.getByTestId('help-write-page')).toContainText('提示词编写建议');

    await page.getByRole('link', { name: '← 返回帮助中心' }).click();
    await expect(page.getByTestId('help-page')).toBeVisible();

    expect(errors).toEqual([]);
  });
});
