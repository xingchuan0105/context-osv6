import { readFileSync } from 'node:fs';
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

async function gotoAdmin(
  page: Page,
  path: string,
  options?: { token?: string; apiBase?: string },
) {
  const { token, apiBase = FIXTURE_BASE } = options ?? {};
  if (token) {
    await seedNextAuth(page, token);
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, apiBase);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.beforeEach(async ({ request }) => {
  await request.post(`${FIXTURE_BASE}/admin/reset`);
});

test.describe('管理后台门禁与概览（E3.5）', () => {
  test('未登录访问 /admin 跳转登录页并携带 next 回跳参数', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoAdmin(page, '/admin');
    await expect(page).toHaveURL(/\/login\?next=\/admin$/);
    expect(errors).toEqual([]);
  });

  test('非 admin 账号访问管理页展示无权访问面板', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*403/]);
    await gotoAdmin(page, '/admin/health', {
      token: 'poc-test-token',
      apiBase: `${FIXTURE_BASE}/case/403-admin`,
    });
    await expect(page.getByTestId('admin-forbidden')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-forbidden')).toContainText('无权访问');
    expect(errors).toEqual([]);
  });

  test('未知管理路由进入 not-found 兜底', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*404/]);
    await gotoAdmin(page, '/admin/nope', { token: 'poc-test-token' });
    await expect(page.locator('main.chat-not-found')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('概览入口网格覆盖全部管理目的地', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoAdmin(page, '/admin', { token: 'poc-test-token' });
    await expect(page.getByTestId('admin-overview')).toBeVisible();
    await expect(page.getByTestId('admin-overview-grid')).toBeVisible({ timeout: 15_000 });
    for (const id of [
      'accounts',
      'users',
      'usage',
      'billing',
      'health',
      'rag-health',
      'workers',
      'degradation',
      'broadcast',
      'audit-logs',
      'feature-flags',
    ]) {
      await expect(page.getByTestId(`admin-entry-${id}`)).toBeVisible();
    }
    expect(errors).toEqual([]);
  });
});

test.describe('管理后台业务闭环（E3.5）', () => {
  test('账户分页、详情封禁、用户删除形成闭环', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoAdmin(page, '/admin/accounts', { token: 'poc-test-token' });

    // 1. 账户列表：12 个账户，每页 10 条
    await expect(page.getByTestId('admin-accounts-table')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-accounts-total')).toContainText('12');
    await expect(page.getByTestId('admin-accounts-page-label')).toContainText('第 1 / 2 页');
    await expect(page.getByTestId('admin-account-row')).toHaveCount(10);

    // 2. 客户端分页翻页
    await page.getByTestId('admin-accounts-next').click();
    await expect(page.getByTestId('admin-account-row')).toHaveCount(2);
    await page.getByTestId('admin-accounts-prev').click();
    await expect(page.getByTestId('admin-account-row')).toHaveCount(10);

    // 3. 账户详情并封禁
    await page.getByTestId('admin-account-detail-link').first().click();
    await expect(page.getByTestId('admin-account-name')).toContainText('Acme 研发中心', {
      timeout: 15_000,
    });
    await expect(page.getByTestId('admin-account-status')).toContainText('正常');
    await page.getByTestId('admin-account-block-btn').click();
    await expect(page.getByTestId('admin-account-status')).toContainText('已封禁', {
      timeout: 15_000,
    });
    await expect(page.getByTestId('admin-account-block-btn')).toContainText('解除封禁');

    // 4. 从详情进入用户列表并删除用户（两步确认）
    await page.getByTestId('admin-account-users-link').click();
    await expect(page.getByTestId('admin-users-table')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-user-row')).toHaveCount(2);
    await page.getByTestId('admin-user-delete-u-2').click();
    await expect(page.getByTestId('admin-user-delete-u-2')).toContainText('确认删除');
    await page.getByTestId('admin-user-delete-u-2').click();
    await expect(page.getByTestId('admin-user-row')).toHaveCount(1);

    expect(errors).toEqual([]);
  });

  test('审计日志筛选、翻页、空态与 CSV 导出', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoAdmin(page, '/admin/audit-logs', { token: 'poc-test-token' });

    await expect(page.getByTestId('admin-audit-table')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-audit-row')).toHaveCount(3);
    await expect(page.getByTestId('admin-audit-total')).toContainText('共 3 条');

    // 1. action 过滤
    await page.getByTestId('admin-audit-filter-action').fill('user.delete');
    await page.getByTestId('admin-audit-submit').click();
    await expect(page.getByTestId('admin-audit-row')).toHaveCount(1);

    // 2. 无匹配 → 空态
    await page.getByTestId('admin-audit-filter-action').fill('user.import');
    await page.getByTestId('admin-audit-submit').click();
    await expect(page.getByTestId('admin-audit-empty')).toBeVisible({ timeout: 15_000 });

    // 3. 时间窗过滤（近 24 小时仅含最近一条）
    await page.getByTestId('admin-audit-filter-action').fill('');
    await page.getByTestId('admin-audit-filter-window').selectOption('24h');
    await page.getByTestId('admin-audit-submit').click();
    await expect(page.getByTestId('admin-audit-row')).toHaveCount(1);

    // 4. CSV 导出
    await page.getByTestId('admin-audit-filter-window').selectOption('');
    await page.getByTestId('admin-audit-submit').click();
    await expect(page.getByTestId('admin-audit-row')).toHaveCount(3);
    const downloadPromise = page.waitForEvent('download');
    await page.getByTestId('admin-audit-csv').click();
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe('audit-logs.csv');
    const csv = readFileSync(await download.path(), 'utf8');
    expect(csv).toContain('account.block');

    expect(errors).toEqual([]);
  });

  test('公告广播提交后回显送达数量', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoAdmin(page, '/admin/broadcast', { token: 'poc-test-token' });

    await expect(page.getByTestId('admin-broadcast-title')).toBeVisible();
    await page.getByTestId('admin-broadcast-title').fill('服务升级通知');
    await page.getByTestId('admin-broadcast-body').fill('今晚 02:00 进行例行升级，预计 30 分钟。');
    await page.getByTestId('admin-broadcast-submit').click();
    await expect(page.getByTestId('admin-broadcast-result')).toContainText('1', {
      timeout: 15_000,
    });
    expect(errors).toEqual([]);
  });

  test('功能开关列表、变更请求提交与复核执行', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoAdmin(page, '/admin/feature-flags', { token: 'poc-test-token' });

    await expect(page.getByTestId('admin-flags-table')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-flag-row')).toHaveCount(2);
    await expect(page.getByTestId('admin-flag-requests')).toBeVisible();
    await expect(page.getByTestId('admin-flag-request-row')).toHaveCount(1);

    // 1. 提交一条新的变更请求
    await page.getByTestId('admin-flag-req-key').fill('rag.offline');
    await page.getByTestId('admin-flag-req-reason').fill('离线检索临时关闭');
    await page.getByTestId('admin-flag-req-submit').click();
    await expect(page.getByTestId('admin-flag-req-result')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('admin-flag-request-row')).toHaveCount(2);

    // 2. 复核通过既有请求 → 开关生效
    await page.getByTestId('admin-flag-approve-req-1').click();
    await expect(page.getByTestId('admin-flag-request-row')).toHaveCount(2);
    await expect(page.getByTestId('admin-flags-table')).toContainText('rag.offline');
    await expect(page.getByTestId('admin-flag-requests')).toContainText('approved');
    expect(errors).toEqual([]);
  });
});
