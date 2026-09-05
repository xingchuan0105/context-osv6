import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const WEB_BASE = `http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}`;
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

async function gotoShare(page: Page, path: string, token?: string) {
  if (token) {
    await seedNextAuth(page, token);
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('工作区分享中心与子页面（E3.3）', () => {
  test('展示分享中心链接、复制操作、访问日志与互动分析', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    const errors = collectPageErrors(page);
    await gotoShare(page, '/dashboard/ws-materials/share', 'poc-test-token');

    // 1. 分享中心呈现与复制链接
    await expect(page.getByTestId('workspace-share-page')).toBeVisible();
    await expect(page.getByTestId('share-active-box')).toBeVisible();
    await expect(page.getByText('/shared/kb/tok-valid-123')).toBeVisible();

    const copyBtn = page.getByTestId('copy-share-btn');
    await copyBtn.click();
    await expect(copyBtn).toHaveText('已复制！');

    // 2. 访问审计日志子页面
    await gotoShare(page, '/dashboard/ws-materials/share/access-logs', 'poc-test-token');
    await expect(page.getByTestId('share-logs-page')).toBeVisible();
    await expect(page.getByText('visitor-88')).toBeVisible();

    // 3. 分享互动分析子页面
    await gotoShare(page, '/dashboard/ws-materials/share/analytics', 'poc-test-token');
    await expect(page.getByTestId('share-analytics-page')).toBeVisible();
    await expect(page.getByText('45')).toBeVisible();

    expect(errors).toEqual([]);
  });
});

test.describe('公开知识库与分享者主页（E3.3）', () => {
  test('正常 Token 展示公开资料库与分享者卡片；失效 Token 展示友好提示', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);

    // 1. 正常 Token 访问
    await gotoShare(page, '/shared/kb/tok-valid-123');
    await expect(page.getByTestId('shared-kb-page')).toBeVisible();
    await expect(page.getByTestId('shared-kb-header')).toContainText('公开材料知识库');
    await expect(page.getByTestId('owner-card')).toContainText('公开分享者');
    await expect(page.getByTestId('shared-source-item')).toContainText('titanium-guide.pdf');

    // 2. 失效 Token 访问
    await gotoShare(page, '/shared/kb/tok-expired');
    await expect(page.getByTestId('share-expired')).toBeVisible();
    await expect(page.getByTestId('share-expired')).toContainText('已失效或不存在');

    expect(errors).toEqual([]);
  });

  test('分享者公开主页呈现与未公开开关语义', async ({ page }) => {
    const errors = collectPageErrors(page);

    // 1. 开启主页展示
    await gotoShare(page, '/shared/u/u-materials-lead');
    await expect(page.getByTestId('user-profile-card')).toBeVisible();
    await expect(page.getByText('李工')).toBeVisible();
    await expect(page.getByText('高级材料工程师')).toBeVisible();

    // 2. 未开启主页展示
    await gotoShare(page, '/shared/u/u-disabled');
    await expect(page.getByTestId('profile-disabled')).toBeVisible();
    await expect(page.getByTestId('profile-disabled')).toContainText('尚未开启公开主页展示');

    expect(errors).toEqual([]);
  });
});

test.describe('工作区邀请加入页面（E3.3）', () => {
  test('未登录引导登录，已登录展示确认接受按钮并直达工作台', async ({ page }) => {
    const errors = collectPageErrors(page);

    // 1. 未登录访客打开邀请链接
    await gotoShare(page, '/invite/ws-materials/mem-99');
    await expect(page.getByTestId('invite-page')).toBeVisible();
    await expect(page.getByTestId('invite-login-btn')).toBeVisible();

    // 2. 已登录用户打开邀请链接并接受
    await gotoShare(page, '/invite/ws-materials/mem-99', 'poc-test-token');
    const acceptBtn = page.getByTestId('accept-invite-btn');
    await expect(acceptBtn).toBeVisible();
    await acceptBtn.click();

    await expect(page).toHaveURL(/\/dashboard\/ws-materials$/);

    expect(errors).toEqual([]);
  });
});
