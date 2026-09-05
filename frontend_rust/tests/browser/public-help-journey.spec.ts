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

test.describe('公开帮助页（E4.1）', () => {
  test('FAQ 页渲染 12 条问答与证据区，SEO 头包含 canonical', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help/faq', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-faq-page')).toBeVisible();
    await expect(page.getByRole('heading', { level: 1 })).toContainText('Context OS 常见问题');
    await expect(page.getByTestId('help-faq-body').locator('h2')).toHaveCount(13);
    await expect(page.getByTestId('help-faq-body')).toContainText('MCP 工具是什么？');

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/help/faq');
    await expect(page.locator('link[rel="alternate"][hreflang="en"]')).toHaveAttribute(
      'href',
      '/en/help/faq',
    );
    expect(await page.title()).toContain('常见问题');

    expect(errors).toEqual([]);
  });

  test('对比页渲染能力对照表与下一步互链', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help/compare', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-compare-page')).toBeVisible();
    await expect(page.getByTestId('help-compare-table')).toBeVisible();
    await expect(page.getByTestId('help-compare-table').locator('tbody tr')).toHaveCount(6);
    await expect(page.getByTestId('help-compare-body')).toContainText('Owner-pays');

    expect(errors).toEqual([]);
  });

  test('API 接入页与 Agent 文档页互链可达', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help/api-access', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('help-api-access-page')).toBeVisible();
    await expect(page.getByTestId('api-access-overview')).toBeVisible();
    await expect(page.getByTestId('api-access-automation')).toBeVisible();

    await page.getByTestId('api-access-automation').getByRole('link').first().click();
    await expect(page.getByTestId('help-agent-api-page')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('agent-api-doc-body')).toBeVisible();
    await expect(page.getByTestId('agent-api-doc-body').locator('table').first()).toBeVisible();
    await expect(page.getByTestId('agent-api-doc-body')).toContainText('workspace.rag_query');

    expect(errors).toEqual([]);
  });
});

test.describe('集成承接页（E4.1）', () => {
  test('索引页三张承接卡与证据互链', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/integrations', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('integrations-page')).toBeVisible();
    await expect(page.getByTestId('integrations-card-mcp')).toBeVisible();
    await expect(page.getByTestId('integrations-card-cursor')).toBeVisible();
    await expect(page.getByTestId('integrations-card-claude-desktop')).toBeVisible();
    await expect(page.getByTestId('integrations-index-body')).toContainText('Agent API 文档');

    expect(errors).toEqual([]);
  });

  test('Cursor 承接页渲染步骤与配置代码块，可返回索引', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/integrations', { waitUntil: 'domcontentloaded' });

    await page.getByTestId('integrations-card-cursor').click();
    await expect(page.getByTestId('integrations-doc-page')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('integrations-doc-body')).toContainText('方式 A：本地 stdio');
    await expect(page.getByTestId('integrations-doc-body').locator('pre code')).toHaveCount(1);
    await expect(page.getByTestId('integrations-doc-body')).toContainText('CONTEXT_OS_API_BASE');

    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/integrations/cursor');

    await page.getByRole('link', { name: '全部集成' }).click();
    await expect(page.getByTestId('integrations-page')).toBeVisible();

    expect(errors).toEqual([]);
  });

  test('FAQ 证据区可到达集成承接页，形成公开站互链闭环', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help/faq', { waitUntil: 'domcontentloaded' });

    await page.getByRole('link', { name: '集成承接（Cursor / Claude / MCP）' }).click();
    await expect(page.getByTestId('integrations-page')).toBeVisible({ timeout: 15_000 });

    await page.getByTestId('integrations-card-claude-desktop').click();
    await expect(page.getByTestId('integrations-doc-body')).toContainText('claude_desktop_config', {
      timeout: 15_000,
    });

    expect(errors).toEqual([]);
  });
});
