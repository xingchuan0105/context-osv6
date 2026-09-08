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
test('workspace selection enters the request without conversation file uploads', async ({ page }) => {
  await setup(page);
  let body: any;
  page.on('request', request => {
    if (request.method() === 'POST' && new URL(request.url()).pathname.endsWith('/chat')) body = request.postDataJSON();
  });
  await page.goto('/dashboard/ws-materials');
  await expect(page.getByTestId('scope-cap-rag')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByTestId('turn-attachment-tray')).toHaveCount(0);
  await page.getByTestId('doc-select').first().check();
  await page.getByTestId('composer-input').fill('Compare the selected materials');
  await page.getByTestId('send-button').click();
  await expect.poll(() => body).toBeTruthy();
  expect(body.workspace_id).toBe('ws-materials');
  expect(body.capabilities).toContain('rag');
  expect(body.doc_scope).toHaveLength(1);
});

test('a workspace session opened on chat moves to its canonical workspace', async ({ page }) => {
  await setup(page);
  await page.goto('/chat/sess-ws-901');
  await expect(page).toHaveURL(/\/dashboard\/ws-materials\?session=sess-ws-901$/);
  await expect(page.getByTestId('scope-cap-rag')).toBeVisible();
  await expect(page.getByTestId('turn-attachment-tray')).toHaveCount(0);
});

test('public sharing submits the share credential and keeps the visitor on the shared page', async ({ page }) => {
  await page.addInitScript(base => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  let body: any;
  page.on('request', request => {
    if (request.method() === 'POST' && new URL(request.url()).pathname.endsWith('/chat')) body = request.postDataJSON();
  });
  await page.goto('/shared/kb/tok-valid-123');
  await page.getByTestId('composer-input').fill('Summarize the shared material');
  await page.getByTestId('send-button').click();
  await expect.poll(() => body).toBeTruthy();
  expect(body.source_type).toBe('share');
  expect(body.source_token).toBe('tok-valid-123');
  await expect(page).toHaveURL(/\/shared\/kb\/tok-valid-123$/);
  await expect(page.getByTestId('chat-rail-toggle')).toHaveCount(0);
});

test('anonymous verification blocks sending and renews its token before retry', async ({ page }) => {
  const bodies: any[] = [];
  await page.route(/\/shared\/kb\/tok-valid-123$/, async route => {
    if (route.request().resourceType() !== 'document') return route.continue();
    const response = await route.fetch();
    await route.fulfill({ response, body: (await response.text()).replace('<head>', '<head><meta name="turnstile-site-key" content="test-site-key">') });
  });
  await page.addInitScript(base => {
    const win = window as any;
    win.__POC_CHAT_API_BASE__ = base;
    let sequence = 0;
    const hosts = new Map<string, Element>();
    win.turnstile = {
      render(host: Element, options: any) {
        const id = String(++sequence);
        hosts.set(id, host);
        const solve = document.createElement('button');
        solve.textContent = 'Solve fixture challenge';
        solve.onclick = () => options.callback(`verified-${id}`);
        host.append(solve);
        return id;
      },
      remove(id: string) { hosts.get(id)?.replaceChildren(); hosts.delete(id); },
    };
  }, FIXTURE_BASE);
  page.on('request', request => {
    if (request.method() === 'POST' && new URL(request.url()).pathname.endsWith('/chat')) bodies.push(request.postDataJSON());
  });
  await page.goto('/shared/kb/tok-valid-123');
  await page.getByTestId('composer-input').fill('Read the shared material');
  await expect(page.getByTestId('send-button')).toBeDisabled();
  await page.getByRole('button', { name: 'Solve fixture challenge' }).click();
  await page.getByTestId('send-button').click();
  await expect(page.getByTestId('status-line')).toHaveText('已完成');
  expect(bodies[0].turnstile_token).toBe('verified-1');
  await expect(page.getByTestId('retry-button')).toHaveCount(0);
  await page.getByRole('button', { name: 'Solve fixture challenge' }).click();
  await page.getByTestId('retry-button').click();
  await expect.poll(() => bodies.length).toBe(2);
  expect(bodies[1].turnstile_token).toBe('verified-2');
});
