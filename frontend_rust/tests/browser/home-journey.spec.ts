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

test.describe('首页产品根（E4.3）', () => {
  test.use({ javaScriptEnabled: false });

  test('无 JS 时 SSR 输出产品价值主张、CTA 与结构化数据', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/', { waitUntil: 'domcontentloaded' });

    await expect(page.getByTestId('home-page')).toBeVisible();
    await expect(page.getByRole('heading', { level: 1 })).toContainText('可本地部署的个人 AI 知识库');
    await expect(page.getByTestId('home-page')).toContainText('文档入库与问答');
    await expect(page.getByTestId('home-page')).toContainText('核心优势与差异化定位');
    await expect(page.getByRole('link', { name: '进入应用' })).toHaveAttribute('href', '/chat');

    // canonical + hreflang + JSON-LD（SSR HTML 静态可达，无需 JS；script 内容用 textContent 断言）
    const canonical = page.locator('link[rel="canonical"]');
    await expect(canonical).toHaveAttribute('href', '/');
    const jsonLd = page.locator('script[type="application/ld+json"]');
    expect(await jsonLd.count()).toBeGreaterThanOrEqual(3);
    const firstLd = await jsonLd.first().evaluate((el) => el.textContent ?? '');
    expect(firstLd).toContain('ContextLM');
    const lastLd = await jsonLd.last().evaluate((el) => el.textContent ?? '');
    expect(lastLd).toContain('SoftwareApplication');

    expect(errors).toEqual([]);
  });
});

test.describe('首页入口路由（E4.3）', () => {
  test('未登录访问 / 跳转 /login', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/', { waitUntil: 'domcontentloaded' });
    await expect(page).toHaveURL(/\/login$/, { timeout: 15_000 });
    expect(errors).toEqual([]);
  });

  test('已登录会话访问 / 跳转 /chat', async ({ page }) => {
    const errors = collectPageErrors(page);
    await seedNextAuth(page, 'poc-test-token');
    await page.addInitScript((base) => {
      (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
    }, FIXTURE_BASE);
    await page.goto('/', { waitUntil: 'domcontentloaded' });
    await expect(page).toHaveURL(/\/chat$/, { timeout: 15_000 });
    expect(errors).toEqual([]);
  });
});

test.describe('抓取协议与站长验证（E4.3）', () => {
  test('/llms.txt 输出 agent 可读站点索引', async ({ request }) => {
    const response = await request.get('/llms.txt');
    expect(response.status()).toBe(200);
    expect(response.headers()['content-type']).toContain('text/plain');
    const body = await response.text();
    expect(body).toContain('# Context OS by ContextLM');
    expect(body).toContain('https://app.contextlm.top/help/api-access/agents');
    expect(body).toContain('https://app.contextlm.top/integrations/mcp');
  });

  test('百度站长文件验证返回指定位串', async ({ request }) => {
    const response = await request.get('/baidu_verify_codeva-THd6TRYMwv.html');
    expect(response.status()).toBe(200);
    expect(await response.text()).toBe('c3054d0735912577ce4407383c2a7965');
  });

  test('FAQ 页挂载 FAQPage 结构化数据', async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto('/help/faq', { waitUntil: 'domcontentloaded' });
    const ldTexts = await page
      .locator('script[type="application/ld+json"]')
      .evaluateAll((scripts) => scripts.map((s) => s.textContent ?? ''));
    const faqLd = ldTexts.find((text) => text.includes('FAQPage'));
    expect(faqLd).toBeTruthy();
    expect(faqLd).toContain('MCP 工具是什么？');
    expect(errors).toEqual([]);
  });
});
