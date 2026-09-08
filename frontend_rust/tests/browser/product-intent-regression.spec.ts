import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

async function setup(page: Page) {
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(base => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
}

test('profile save persists the server response and preserves other profile fields', async ({ page }) => {
  await setup(page);
  let profile = { id: 'fixture-user', email: 'poc@example.com', full_name: 'PoC', bio: 'Existing bio', contact_url: 'https://example.com', public_profile_enabled: true };
  await page.route('**/api/auth/me', route => route.fulfill({ json: { success: true, data: { user: profile } } }));
  let writes = 0;
  await page.route('**/api/auth/profile', async route => {
    expect(route.request().method()).toBe('PUT');
    const body = route.request().postDataJSON();
    expect(body).toEqual({ full_name: 'New Name', bio: 'Existing bio', contact_url: 'https://example.com', public_profile_enabled: true });
    writes++;
    profile = { ...profile, ...body };
    await route.fulfill({ json: { success: true, data: { token: '', user: profile } } });
  });
  await page.goto('/settings');
  await expect(page.getByTestId('profile-panel')).toBeVisible();
  await page.getByTestId('profile-name').fill('New Name');
  await page.getByTestId('profile-save').click();
  await expect(page.getByTestId('app-toast')).toBeVisible();
  expect(writes).toBe(1);
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem('avrag.auth.v1')!).user.full_name)).toBe('New Name');
  // Navigate away and back without reseeding localStorage; readback uses the saved account.
  await page.getByTestId('tab-providers').click();
  await page.getByTestId('tab-profile').click();
  await expect(page.getByTestId('profile-name')).toHaveValue('New Name');
});

test('failed profile save preserves input without a success toast', async ({ page }) => {
  await setup(page);
  await page.route('**/api/auth/profile', route => route.fulfill({ status: 500, json: { error: 'save failed' } }));
  await page.goto('/settings');
  await page.getByTestId('profile-name').fill('Unsaved name');
  await page.getByTestId('profile-save').click();
  await expect(page.getByRole('alert')).toContainText('资料保存失败');
  await expect(page.getByTestId('profile-name')).toHaveValue('Unsaved name');
  await expect(page.getByTestId('app-toast')).toBeHidden();
});

test('failed invitation acceptance stays on the invitation and supports retry', async ({ page }) => {
  await setup(page);
  let attempts = 0;
  await page.route('**/api/v1/workspaces/ws-materials/members/mem-99/accept', async route => {
    attempts++;
    await route.fulfill({ status: attempts === 1 ? 403 : 200, json: { ok: attempts > 1 } });
  });
  await page.goto('/invite/ws-materials/mem-99');
  await page.getByTestId('accept-invite-btn').click();
  await expect(page.getByTestId('invite-error')).toBeVisible();
  await expect(page).toHaveURL(/\/invite\/ws-materials\/mem-99$/);
  await page.getByTestId('accept-invite-btn').click();
  await expect(page).toHaveURL(/\/dashboard\/ws-materials$/);
  expect(attempts).toBe(2);
});

for (const [status, expected] of [
  ['pending', '尚未确认付款'], ['paid', '已确认此订单支付成功'],
  ['failed', '此订单未完成'], ['unexpected', '支付结果尚未确认'],
] as const) {
  test(`payment return renders server status ${status}`, async ({ page }) => {
    await setup(page);
    await page.route('**/api/v1/billing/orders/ord-check', route => route.fulfill({ json: { order_id: 'ord-check', status, plan_id: 'pro' } }));
    await page.goto('/upgrade/success?order_id=ord-check');
    await expect(page.getByTestId('payment-status')).toContainText(expected);
    if (status !== 'paid') await expect(page.getByTestId('upgrade-success-page')).not.toContainText('支付成功');
  });
}

test('payment without order and verification failure never claim success', async ({ page }) => {
  await setup(page);
  await page.goto('/upgrade/success');
  await expect(page.getByTestId('payment-status')).toContainText('缺少订单号');
  await page.route('**/api/v1/billing/orders/ord-check', route => route.fulfill({ status: 503, json: { error: 'unavailable' } }));
  await page.goto('/upgrade/success?order_id=ord-check');
  await expect(page.getByTestId('payment-status')).toContainText('无法核实订单');
  await expect(page.getByTestId('upgrade-success-page')).not.toContainText('支付成功');
});

test('saving and revoking one LLM provider preserves another provider with the same purpose', async ({ page }) => {
  await setup(page);
  const agent = { id: 'server-agent-id', provider: 'deepseek', purpose: 'llm', model_hint: 'deepseek-v4-flash', is_active: true, revoked_at: null };
  let secrets = [agent];
  await page.route('**/api/v1/settings/provider-secrets', async route => {
    if (route.request().method() === 'PUT') {
      const body = route.request().postDataJSON();
      expect(body.provider).toBe('bailian');
      secrets = [...secrets, { ...agent, id: 'server-parse-id', provider: body.provider, model_hint: body.model_hint }];
    }
    await route.fulfill({ json: { secrets } });
  });
  await page.route('**/api/v1/settings/provider-secrets/server-parse-id', async route => {
    expect(route.request().method()).toBe('DELETE');
    secrets = secrets.filter(secret => secret.id !== 'server-parse-id');
    await route.fulfill({ json: { ok: true } });
  });
  await page.goto('/settings?tab=providers');
  await expect(page.getByTestId('status-agent_llm')).toBeVisible();
  await page.getByTestId('input-parse_llm').fill('fixture-placeholder-key');
  await page.getByTestId('save-parse_llm').click();
  await expect(page.getByTestId('status-parse_llm')).toBeVisible();
  await expect(page.getByTestId('status-agent_llm')).toBeVisible();
  await page.getByTestId('revoke-parse_llm').click();
  await expect(page.getByTestId('input-parse_llm')).toBeVisible();
  await expect(page.getByTestId('status-agent_llm')).toBeVisible();
  expect(secrets).toEqual([agent]);
});

test('clipboard rejection reports failure and leaves a full address for manual copying', async ({ page }) => {
  await setup(page);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: () => Promise.reject(new Error('denied')) } });
  });
  await page.goto('/dashboard/ws-materials/share');
  await page.getByTestId('copy-share-btn').click();
  await expect(page.getByTestId('share-error')).toContainText('复制失败');
  await expect(page.getByTestId('copy-share-btn')).not.toContainText('已复制');
  await expect(page.getByTestId('share-url')).toHaveText(new URL('/shared/kb/tok-valid-123', page.url()).href);
});
