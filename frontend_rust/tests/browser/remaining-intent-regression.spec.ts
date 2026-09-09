import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;
async function setup(page: Page) {
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, BASE);
}
test.beforeEach(async ({ page, request }) => {
  await request.post(`${BASE}/admin/reset`);
  await setup(page);
});

test('membership checkout uses the selected real plan and never a topup pack', async ({ page }) => {
  let body: any;
  await page.route('**/api/v1/billing/checkout-session', async route => {
    body = route.request().postDataJSON();
    await route.fulfill({ json: { url: 'https://example.com/pay', session_id: 'checkout' } });
  });
  await page.goto('/pricing');
  await page.getByTestId('pricing-agree').check();
  const buy = page.getByTestId('plan-pro').getByTestId('subscribe-plan');
  await expect(buy).toBeEnabled();
  await buy.click();
  await expect(page.getByTestId('pay-qr-dialog')).toBeVisible();
  expect(body).toMatchObject({ kind: 'subscription', plan_id: 'pro', provider: 'alipay' });
  expect(body).not.toHaveProperty('topup_pack_id');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('pay-qr-dialog')).not.toBeVisible();
  await expect(buy).toBeFocused();
});

test('failed price and wallet reads do not fabricate prices or zero balance', async ({ page }) => {
  await page.route('**/api/v1/billing/plans', route => route.fulfill({ status: 503, body: 'plans unavailable' }));
  await page.route('**/api/v1/billing/wallet', route => route.fulfill({ status: 503, body: 'wallet unavailable' }));
  await page.goto('/pricing');
  await expect(page.getByTestId('plans-fallback')).toBeVisible();
  await expect(page.getByTestId('plan-pro')).toHaveCount(0);
  await expect(page.getByTestId('wallet-balance')).not.toContainText('¥0');
  await expect(page.getByTestId('pricing-retry')).toBeEnabled();
});

test('workspace load failure is recoverable and does not claim an empty knowledge base', async ({ page }) => {
  let fail = true;
  await page.route('**/api/v1/documents?workspace_id=ws-materials', route => fail
    ? route.fulfill({ status: 503, body: 'documents unavailable' }) : route.continue());
  await page.goto('/dashboard/ws-materials');
  await expect(page.getByTestId('workbench-load-error')).toBeVisible();
  await expect(page.getByTestId('sources-empty')).not.toBeVisible();
  fail = false;
  await page.getByTestId('workbench-load-error').getByRole('button').click();
  await page.getByTestId('workspace-open-sources').click();
  await expect(page.getByTestId('workspace-doc-item').first()).toBeVisible();
  await expect(page.getByTestId('workbench-load-error')).not.toBeVisible();
});

test('document preview opens actual content and Escape restores focus', async ({ page }) => {
  await page.route('**/api/v1/documents/*/content', route => route.fulfill({ json: { content: 'A real document preview', summary: null } }));
  await page.goto('/dashboard/ws-materials');
  const open = page.getByTestId('preview-doc').first();
  await page.getByTestId('workspace-open-sources').click();
  await open.click();
  await expect(page).toHaveURL(/source=/);
  const dialog = page.getByTestId('source-preview');
  await expect(dialog).toContainText('A real document preview');
  await page.keyboard.press('Tab');
  // The browser may move to its own chrome after the last control; it must not
  // focus controls in the inert page behind the native dialog.
  expect(await dialog.evaluate(el => document.activeElement === document.body || el.contains(document.activeElement))).toBe(true);
  await open.focus();
  await expect(open).not.toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(open).toBeFocused();
});

test('failed source import keeps the dialog and text for retry', async ({ page }) => {
  await page.route('**/api/v1/workspaces/ws-materials/sources/paste', route => route.fulfill({ status: 503, body: 'import failed' }));
  await page.goto('/dashboard/ws-materials');
  await page.getByTestId('workspace-open-sources').click();
  await page.getByTestId('preview-doc').first().waitFor();
  await page.getByTestId('open-upload').click();
  await page.getByTestId('upload-tab-paste').click();
  await page.getByTestId('workspace-paste-title').fill('Keep this title');
  await page.getByTestId('workspace-paste-body').fill('Keep this text');
  await page.getByTestId('workspace-paste-submit').click();
  await expect(page.getByTestId('upload-dialog')).toBeVisible();
  await expect(page.getByTestId('upload-dialog').getByRole('alert')).toContainText('import failed');
  await expect(page.getByTestId('workspace-paste-body')).toHaveValue('Keep this text');
});

test('failed file transfer never calls upload completion or reports success', async ({ page }) => {
  let completed = 0;
  await page.route('**/upload/*', route => route.fulfill({ status: 503, body: 'transfer failed' }));
  await page.route('**/api/v1/documents/*/complete-upload', route => { completed++; return route.continue(); });
  await page.goto('/dashboard/ws-materials');
  await page.getByTestId('workspace-open-sources').click();
  await page.getByTestId('preview-doc').first().waitFor();
  await page.getByTestId('open-upload').click();
  await page.getByTestId('workspace-file-input').setInputFiles({ name: 'sample.txt', mimeType: 'text/plain', buffer: Buffer.from('Sample text') });
  await page.getByTestId('workspace-file-submit').click();
  await expect(page.getByTestId('upload-dialog').getByRole('alert')).toContainText('transfer failed');
  expect(completed).toBe(0);
  await expect(page.getByTestId('app-toast')).not.toBeVisible();
});

test('notification failure has retry, not an empty inbox message', async ({ page }) => {
  await page.route('**/api/v1/notifications', route => route.fulfill({ status: 503, body: 'notifications unavailable' }));
  await page.goto('/dashboard/ws-materials');
  await expect(page.getByTestId('workspace-quick-add')).toBeEnabled();
  await page.getByTestId('notification-bell').click();
  await expect(page.getByTestId('notification-error')).toBeVisible();
  await expect(page.getByTestId('notification-empty')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('notification-bell')).toBeFocused();
});

test('share analytics failure displays unknown rather than zero', async ({ page }) => {
  await page.route('**/api/v1/workspaces/ws-materials/share/analytics', route => route.fulfill({ status: 503, body: 'statistics unavailable' }));
  await page.goto('/dashboard/ws-materials/share/analytics');
  await expect(page.getByRole('alert')).toContainText('statistics unavailable');
  await expect(page.locator('.settings-usage-value').first()).toHaveText(/未知|Unknown/);
});

test('pin waits for the write and refreshes only after the server confirms it', async ({ page }) => {
  let pinned = false;
  let storedSession: any;
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  await page.route('**/api/v1/chat/sessions', async route => {
    const response = await route.fetch();
    const json = await response.json();
    const data = json.data ?? json;
    data.sessions = data.sessions.map((session: any) => ({ ...session, pinned: session.id === 'sess-ws-901' && pinned }));
    storedSession = data.sessions.find((session: any) => session.id === 'sess-ws-901');
    await route.fulfill({ json });
  });
  await page.route('**/api/v1/chat/sessions/sess-ws-901', async route => {
    if (route.request().method() === 'PATCH' || route.request().method() === 'PUT') {
      await gate;
      pinned = true;
      await route.fulfill({ json: { ...storedSession, pinned: true } });
    } else await route.continue();
  });
  await page.goto('/dashboard/ws-materials');
  const pin = page.getByTestId('pin-session').first();
  await pin.click();
  await expect(pin).toBeDisabled();
  expect(pinned).toBe(false);
  release();
  await expect(pin).toContainText(/取消置顶|Unpin/);
});

test('sharing labels follow the account language setting', async ({ page }) => {
  await page.goto('/dashboard/ws-materials/share');
  await expect(page.getByTestId('share-active-box')).toBeVisible();
  await page.getByTestId('dashboard-account-menu-trigger').click();
  await page.getByTestId('account-locale-toggle').click();
  await page.getByTestId('account-locale-en').click();
  await expect(page.getByRole('heading', { name: 'Workspace sharing', exact: true })).toBeVisible();
  await expect(page.getByTestId('revoke-share-btn')).toHaveText('Stop sharing');
});

test('global analytics labels summed UV and suppresses incomplete totals', async ({ page }) => {
  await page.goto('/dashboard/analytics');
  await expect(page.getByTestId('analytics-visitors')).toBeVisible();
  await expect(page.getByText(/跨库未去重|not deduplicated across workspaces/)).toBeVisible();
  await page.route('**/api/v1/workspaces/*/share/analytics', route => route.fulfill({ status: 503, body: 'statistics unavailable' }));
  await page.reload();
  await expect(page.getByTestId('page-error')).toBeVisible();
  await expect(page.getByTestId('analytics-visitors')).not.toBeVisible();
});
