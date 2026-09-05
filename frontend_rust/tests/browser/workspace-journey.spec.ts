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

async function gotoDashboard(page: Page, path = '/dashboard') {
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('工作区概览与创建（E3.2）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('展示工作区卡片，支持弹窗创建新工作区并跳转工作台', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoDashboard(page, '/dashboard');

    // 1. 验证既有工作区卡片展示
    const card = page.getByTestId('workspace-card').filter({ hasText: '材料研发' });
    await expect(card).toBeVisible({ timeout: 15_000 });
    await expect(card).toContainText('材料研发知识库');
    await expect(card).toContainText('1 篇资料');

    // 2. 点击新建工作区
    await page.getByTestId('create-workspace-btn').click();
    await expect(page.getByRole('dialog', { name: '新建工作区' })).toBeVisible();

    await page.getByTestId('new-workspace-name').fill('深海探测器资料库');
    await page.getByTestId('new-workspace-desc').fill('耐压壳体与动力推进');
    await page.getByTestId('submit-create-workspace').click();

    // 3. 创建成功后自动跳转到该新工作台
    await expect(page).toHaveURL(/\/dashboard\/ws-/);
    await expect(page.getByTestId('workspace-workbench')).toBeVisible();

    expect(errors).toEqual([]);
  });
});

test.describe('工作区工作台控制台与持久资料/笔记（E3.2）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('工作台右轨展示持久文件与笔记，支持删除文档与添加笔记', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoDashboard(page, '/dashboard/ws-materials');

    // 1. 验证工作台外壳与侧栏
    await expect(page.getByTestId('workspace-workbench')).toBeVisible();
    await expect(page.getByTestId('workspace-side-rail')).toBeVisible();

    // 2. 验证持久文件列表与删除操作
    const docItem = page.getByTestId('workspace-doc-item').filter({ hasText: 'titanium-spec.pdf' });
    await expect(docItem).toBeVisible({ timeout: 15_000 });
    await expect(docItem).toContainText('completed');

    await docItem.getByTestId('delete-doc-btn').click();
    await expect(page.getByTestId('workspace-doc-item')).toHaveCount(0, { timeout: 15_000 });

    // 3. 切换到工作区笔记 Tab
    await page.getByTestId('tab-notes').click();
    const noteItem = page.getByTestId('workspace-note-item').filter({ hasText: '周会讨论要点' });
    await expect(noteItem).toBeVisible();

    // 4. 添加新工作区笔记
    await page.getByTestId('btn-new-note').click();
    await page.getByTestId('note-title-input').fill('材料选型备忘');
    await page.getByTestId('note-content-input').fill('优先选用 TC4 钛合金棒材。');
    await page.getByTestId('submit-note-btn').click();

    const newNoteItem = page.getByTestId('workspace-note-item').filter({ hasText: '材料选型备忘' });
    await expect(newNoteItem).toBeVisible({ timeout: 15_000 });

    expect(errors).toEqual([]);
  });
});

test.describe('工作区深度分析与全局统计（E3.2）', () => {
  test('切片健康度分析页与全局分享流量统计页正常渲染', async ({ page }) => {
    const errors = collectPageErrors(page);

    // 1. 访问工作区分析页
    await gotoDashboard(page, '/dashboard/ws-materials/analyze');
    await expect(page.getByTestId('workspace-analyze-page')).toBeVisible();
    await expect(page.getByTestId('analyze-panel')).toContainText('ws-materials');
    await expect(page.getByTestId('analyze-panel')).toContainText('42');

    // 2. 访问全局分析页
    await gotoDashboard(page, '/dashboard/analytics');
    await expect(page.getByTestId('global-analytics-page')).toBeVisible();
    await expect(page.getByTestId('global-analytics-page')).toContainText('总公开浏览量');

    expect(errors).toEqual([]);
  });
});
