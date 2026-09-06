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

test.describe('英文公共站（E4.4）', () => {
  // /en 与 / 一样带会话分流（未登录跳 /login）；内容断言在无 JS 下看 SSR 输出。
  test.use({ javaScriptEnabled: false });

  test('/en 渲染英文价值主张与 canonical', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/en', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('home-page')).toBeVisible();
    await expect(page.getByRole('heading', { level: 1 })).toContainText(
      'locally deployable personal AI knowledge base',
    );
    await expect(page.getByTestId('home-page')).toContainText('Documents & Q&A');
    await expect(page.getByRole('link', { name: 'Open the app' })).toHaveAttribute('href', '/chat');

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/en');
    await expect(page.locator('link[rel="alternate"][hreflang="zh-CN"]')).toHaveAttribute(
      'href',
      '/',
    );
    const ldTexts = await page
      .locator('script[type="application/ld+json"]')
      .evaluateAll((scripts) => scripts.map((s) => s.textContent ?? ''));
    expect(ldTexts.some((text) => text.includes('"inLanguage":"en"'))).toBe(true);

    expect(errors).toEqual([]);
  });

  test('/en/desktop 渲染英文文案与 canonical', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await page.goto('/en/desktop', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('desktop-product-page')).toBeVisible();
    await expect(page.getByRole('heading', { level: 1 })).toContainText('desktop client');
    await expect(page.getByTestId('desktop-product-page')).toContainText('Private by default');
    // 无 JS 时下载清单不拉取，仅断言 CTA 行存在（下载态由 desktop-legal-journey 覆盖）
    await expect(page.getByTestId('desktop-cta-row').locator('button, a').first()).toBeVisible();

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/en/desktop');
    await expect(page.locator('link[rel="alternate"][hreflang="zh-CN"]')).toHaveAttribute(
      'href',
      '/desktop',
    );

    expect(errors).toEqual([]);
  });

  test('/en/pricing 复用定价组件并挂 en SEO 头', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await page.goto('/en/pricing', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('pricing-page')).toBeVisible();
    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/en/pricing');
    await expect(page.locator('link[rel="alternate"][hreflang="zh-CN"]')).toHaveAttribute(
      'href',
      '/pricing',
    );

    expect(errors).toEqual([]);
  });

  test('/en/help/faq 渲染英文问答且 hreflang 指回 zh 页', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/en/help/faq', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-faq-page-en')).toBeVisible();
    await expect(page.getByTestId('help-faq-body')).toContainText('What is Context OS?');
    await expect(page.getByTestId('help-faq-body').locator('h2')).toHaveCount(13);
    await expect(page.getByRole('link', { name: 'Free desktop client' })).toHaveAttribute(
      'href',
      '/desktop',
    );

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/en/help/faq');
    await expect(page.locator('link[rel="alternate"][hreflang="zh-CN"]')).toHaveAttribute(
      'href',
      '/help/faq',
    );
    const ldTexts = await page
      .locator('script[type="application/ld+json"]')
      .evaluateAll((scripts) => scripts.map((s) => s.textContent ?? ''));
    expect(ldTexts.some((text) => text.includes('FAQPage') && text.includes('"inLanguage":"en"'))).toBe(
      true,
    );

    expect(errors).toEqual([]);
  });

  test('/en/help/compare 与 /en/help/api-access 渲染英文内容', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/en/help/compare', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-compare-page-en')).toBeVisible();
    await expect(page.getByTestId('help-compare-table').locator('tbody tr')).toHaveCount(6);
    await expect(page.getByTestId('help-compare-body')).toContainText('owner-pays');

    await page.goto('/en/help/api-access', { waitUntil: 'domcontentloaded' });
    await expect(page.getByTestId('help-api-access-page-en')).toBeVisible();
    await expect(page.getByTestId('api-access-overview')).toContainText(
      'Each workspace can create and revoke its own API keys',
    );

    expect(errors).toEqual([]);
  });

  test('/en/legal 法务三页英文内容与互链', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/en/legal', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('legal-center-page-en')).toBeVisible();
    await expect(page.getByTestId('legal-card-terms')).toContainText('Terms of service');
    await expect(page.getByTestId('legal-card-terms')).toHaveAttribute('href', '/en/legal/terms');

    await page.getByTestId('legal-card-terms').click();
    await expect(page.getByTestId('legal-terms-page-en')).toBeVisible({ timeout: 15_000 });
    await expect(
      page.getByTestId('legal-doc-layout').getByRole('heading', { level: 1 }).first(),
    ).toContainText('Terms of Service');

    await page.getByRole('link', { name: 'Back to legal center' }).click();
    await expect(page.getByTestId('legal-center-page-en')).toBeVisible({ timeout: 15_000 });
    await page.getByTestId('legal-card-licenses').click();
    await expect(page.getByTestId('legal-licenses-page-en')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('licenses-table').locator('tbody tr')).toHaveCount(6);

    expect(errors).toEqual([]);
  });

  test('/en/help/api-access/agents 复用英文 agent 文档且 canonical 指向 en', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/en/help/api-access/agents', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-agent-api-page-en')).toBeVisible();
    await expect(page.getByTestId('agent-api-doc-body')).toContainText('workspace.rag_query');

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/en/help/api-access/agents');
    await expect(page.locator('link[rel="alternate"][hreflang="zh-CN"]')).toHaveAttribute(
      'href',
      '/help/api-access/agents',
    );

    expect(errors).toEqual([]);
  });
});
