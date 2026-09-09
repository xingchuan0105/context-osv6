import { test, expect } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const fixture = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

test.beforeEach(async ({ page, request }) => {
  await request.post(`${fixture}/admin/reset`);
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, fixture);
});

for (const theme of ['light', 'dark']) {
for (const width of [390, 768, 1280, 1440]) {
  test(`workspace notes and chat drafts survive panel changes at ${width}px ${theme}`, async ({ page }) => {
    await page.addInitScript(theme => localStorage.setItem('avrag.ui.theme.v1', theme), theme);
    await page.setViewportSize({ width, height: 720 });
    await page.goto('/dashboard/ws-materials');
    const panel = page.getByTestId('workspace-side-rail');
    await expect(panel).toBeHidden();
    await expect(page.getByTestId('workspace-quick-add')).toBeEnabled();
    await page.getByTestId('composer-input').fill('保留工作区问题');
    await page.getByTestId('workspace-open-notes').click();
    await page.getByTestId('btn-new-note').click();
    await page.getByTestId('note-title-input').fill('未保存的笔记');
    await page.locator('.note-editor-host [contenteditable="true"]').fill('切换分区后仍保留正文。');
    await expect(page.getByTestId('note-content-input')).toBeHidden();
    await page.getByTestId('tab-sources').click();
    await expect(page.getByTestId('workspace-doc-item').first()).toBeVisible();
    await page.getByTestId('tab-notes').click();
    await expect(page.getByTestId('note-title-input')).toHaveValue('未保存的笔记');
    await expect(page.locator('.note-editor-host [contenteditable="true"]')).toHaveText('切换分区后仍保留正文。');
    await page.getByTestId('workspace-panel-close').click();
    await expect(panel).toBeHidden();
    await expect(page.getByTestId('composer-input')).toHaveValue('保留工作区问题');
    await expect(page.getByTestId('note-draft-indicator')).toBeVisible();
    await page.getByTestId('workspace-open-notes').click();
    await expect(page.getByTestId('note-title-input')).toHaveValue('未保存的笔记');
    await page.screenshot({ path: `/tmp/context-rust-w2-visual/notes-${width}-${theme}.png`, animations: 'disabled' });
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await expect(page.getByTestId('workspace-open-notes')).toBeFocused();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    const send = await page.getByTestId('send-button').boundingBox();
    expect(send!.y + send!.height).toBeLessThanOrEqual(720);
    await page.screenshot({ path: `/tmp/context-rust-w2-visual/closed-${width}-${theme}.png`, animations: 'disabled' });
  });
}
}

test('note draft recovers after leaving and returning to its workspace', async ({ page }) => {
  await page.goto('/dashboard/ws-materials');
  await page.getByTestId('workspace-open-notes').click();
  await page.getByTestId('btn-new-note').click();
  await page.getByTestId('note-title-input').fill('恢复笔记');
  await page.locator('.note-editor-host [contenteditable="true"]').fill('属于材料工作区');
  await page.getByTestId('workspace-panel-close').click();
  page.once('dialog', dialog => dialog.accept());
  await page.getByTestId('all-workspaces-link').click();
  await expect(page).toHaveURL(/\/dashboard$/);
  await page.goBack();
  await page.getByTestId('workspace-open-notes').click();
  await expect(page.getByTestId('note-title-input')).toHaveValue('恢复笔记');
  await expect(page.locator('.note-editor-host [contenteditable="true"]')).toHaveText('属于材料工作区');
  await page.getByTestId('submit-note-btn').click();
  await expect(page.getByTestId('workspace-note-item').filter({ hasText: '恢复笔记' })).toBeVisible();
  await page.getByTestId('workspace-panel-close').click();
  await expect(page.getByTestId('note-draft-indicator')).toBeHidden();
});

test('quick add starts file selection while the panel is closed and preserves transfer errors', async ({ page }) => {
  let completed = 0;
  await page.route('**/upload/*', route => route.fulfill({ status: 503, body: 'transfer failed' }));
  await page.route('**/api/v1/documents/*/complete-upload', route => { completed++; return route.continue(); });
  await page.goto('/dashboard/ws-materials');
  const chooserPromise = page.waitForEvent('filechooser');
  await page.getByTestId('workspace-quick-add').click();
  const chooser = await chooserPromise;
  await expect(page.getByTestId('workspace-side-rail')).toBeHidden();
  await chooser.setFiles({ name: 'workspace.txt', mimeType: 'text/plain', buffer: Buffer.from('Persistent workspace source') });
  await expect(page.getByTestId('workspace-side-rail')).toBeVisible();
  await expect(page.getByTestId('workbench-action-error')).toContainText('transfer failed');
  expect(completed).toBe(0);
  await expect(page.getByTestId('upload-dialog')).toBeHidden();
});
