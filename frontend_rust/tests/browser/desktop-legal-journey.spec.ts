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

test.describe('桌面产品页与重定向（E4.2）', () => {
  test('/desktop 渲染产品信息、安装步骤与下载态', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await page.goto('/desktop', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('desktop-product-page')).toBeVisible();
    await expect(page.getByRole('heading', { level: 1 })).toContainText('桌面客户端');
    await expect(page.getByTestId('desktop-cta-row')).toBeVisible();
    // 无发布清单 → 未发布态（与 Next 行为一致）
    await expect(page.getByTestId('desktop-download-unavailable').or(page.getByTestId('desktop-download-windows'))).toBeVisible({ timeout: 15_000 });

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/desktop');
    expect(await page.title()).toContain('桌面客户端');

    expect(errors).toEqual([]);
  });

  test('/activate 旧深链重定向到 /desktop', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await page.goto('/activate', { waitUntil: 'domcontentloaded' });
    await expect(page).toHaveURL(/\/desktop$/, { timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('/setup 旧深链重定向到 /settings?tab=providers', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/setup', { waitUntil: 'domcontentloaded' });
    await expect(page).toHaveURL(/\/settings\?tab=providers$/, { timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('FAQ 的「免费客户端」入口指向规范 /desktop', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help/faq', { waitUntil: 'domcontentloaded' });
    await expect(page.getByRole('link', { name: '免费客户端' })).toHaveAttribute('href', '/desktop');
    expect(errors).toEqual([]);
  });
});

test.describe('法律中心（E4.2）', () => {
  test('法律中心三卡与联系方式', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/legal', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('legal-center-page')).toBeVisible();
    await expect(page.getByTestId('legal-card-terms')).toBeVisible();
    await expect(page.getByTestId('legal-card-privacy')).toBeVisible();
    await expect(page.getByTestId('legal-card-licenses')).toBeVisible();
    await expect(page.getByTestId('legal-footer-links')).toBeVisible();

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/legal');

    expect(errors).toEqual([]);
  });

  test('用户服务协议渲染正文、版本与目录锚点', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/legal', { waitUntil: 'domcontentloaded' });

    await page.getByTestId('legal-card-terms').click();
    await expect(page.getByTestId('legal-terms-page')).toBeVisible({ timeout: 15_000 });
    await expect(
      page.getByTestId('legal-doc-layout').getByRole('heading', { level: 1 }).first(),
    ).toContainText('用户服务协议');
    await expect(page.getByTestId('legal-doc-layout')).toContainText('版本：2026-06-13');
    const tocLinks = page.locator('.legal-toc-list a');
    await expect(tocLinks.first()).toBeVisible();
    const firstHref = await tocLinks.first().getAttribute('href');
    expect(firstHref).toMatch(/^#/);

    expect(errors).toEqual([]);
  });

  test('开源声明三页互链：摘要 → MIT 全文 → 第三方声明', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/legal/licenses', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('legal-licenses-page')).toBeVisible();
    await expect(page.getByTestId('licenses-table').locator('tbody tr')).toHaveCount(6);

    await page.getByRole('link', { name: '查看MIT许可证全文' }).click();
    await expect(page.getByTestId('legal-project-license-page')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('legal-project-license-page')).toContainText('MIT License');
    await expect(page.getByTestId('legal-project-license-page')).toContainText('AVRag Team');

    await page.getByRole('link', { name: '返回法律中心' }).click();
    await expect(page.getByTestId('legal-center-page')).toBeVisible({ timeout: 15_000 });
    await page.getByTestId('legal-card-licenses').click();
    await page.getByRole('link', { name: '查看完整第三方声明' }).click();
    await expect(page.getByTestId('legal-third-party-page')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('third-party-total')).toContainText('组件总数');

    // 下载 .md 静态路由
    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('link', { name: '下载 .md' }).click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe('third-party-notices.md');

    expect(errors).toEqual([]);
  });
});
