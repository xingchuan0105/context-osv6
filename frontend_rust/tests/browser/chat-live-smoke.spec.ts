import { test, expect, type APIRequestContext, type Page } from '@playwright/test';

const LIVE_ENABLED = process.env.LIVE_BACKEND === '1';
const API_BASE = (process.env.LIVE_API_BASE || 'http://127.0.0.1:8080').replace(
  /\/$/,
  '',
);
const TERMS_VERSION = '2026-06-13';
const PRIVACY_VERSION = '2026-06-13';

test.skip(!LIVE_ENABLED, 'set LIVE_BACKEND=1 to run against a live avrag-api');

async function gotoChat(page: Page, apiBase: string, path = '/chat') {
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ =
      base;
  }, apiBase);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
  await expect(page.getByTestId('chat-canvas')).toBeVisible();
}

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

type AuthEnvelope = {
  success?: boolean;
  data?: { token?: string } | null;
  error?: string | null;
};

async function obtainLiveJwt(request: APIRequestContext): Promise<string> {
  const existing = process.env.LIVE_JWT?.trim();
  if (existing) {
    return existing;
  }

  const email = process.env.E2E_TEST_USER_EMAIL || 'e2e-test@example.com';
  const password = process.env.E2E_TEST_USER_PASSWORD || 'E2eTest123!';
  const login = await request.post(`${API_BASE}/api/auth/login`, {
    data: { email, password },
  });
  const loginBody = (await login.json().catch(() => ({}))) as AuthEnvelope;
  if (login.ok() && loginBody.data?.token) {
    return loginBody.data.token;
  }

  if (loginBody.error !== 'account_not_registered') {
    throw new Error(`live login failed with HTTP ${login.status()}`);
  }

  const register = await request.post(`${API_BASE}/api/auth/register`, {
    data: {
      email,
      password,
      full_name: 'E2E Test User',
      terms_version: TERMS_VERSION,
      privacy_version: PRIVACY_VERSION,
    },
  });
  const registerBody = (await register.json().catch(() => ({}))) as AuthEnvelope;
  if (register.ok() && registerBody.data?.token) {
    return registerBody.data.token;
  }

  const retry = await request.post(`${API_BASE}/api/auth/login`, {
    data: { email, password },
  });
  const retryBody = (await retry.json().catch(() => ({}))) as AuthEnvelope;
  if (retry.ok() && retryBody.data?.token) {
    return retryBody.data.token;
  }
  throw new Error(`live register/login failed with HTTP ${register.status()}/${retry.status()}`);
}

test.describe('live backend smoke', () => {
  test('无 token 打到真实 API 时显示 unauthorized（401 对照）', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*401/]);
    await gotoChat(page, API_BASE);

    await page.getByTestId('composer-input').fill('401 对照');
    await page.getByTestId('send-button').click();

    const alert = page.getByRole('alert');
    await expect(alert).toBeVisible({ timeout: 15_000 });
    await expect(alert).toContainText('unauthorized');
    await expect(page.getByTestId('live-answer')).toHaveText('');

    expect(errors).toEqual([]);
  });

  test('PoC token 框走一轮真实 Quick Chat', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    const token = await obtainLiveJwt(request);
    await gotoChat(page, API_BASE);

    await page.locator('.poc-token summary').click();
    await page.getByTestId('poc-token-input').fill(token);
    await page.getByTestId('composer-input').fill('只回答：pong');
    await page.getByTestId('send-button').click();

    await expect(page).toHaveURL(/\/chat\/[0-9a-fA-F-]{8,}$/, { timeout: 60_000 });

    const liveAnswer = page.getByTestId('live-answer');
    await expect(liveAnswer).not.toHaveText('', { timeout: 120_000 });
    const answerText = ((await liveAnswer.textContent()) || '').trim();
    expect(answerText.length).toBeGreaterThan(0);

    await expect(page.getByTestId('status-line')).toHaveText('已完成', {
      timeout: 120_000,
    });

    const activity = page.getByTestId('activity-region');
    if (await activity.isVisible()) {
      const activityText = ((await activity.textContent()) || '').trim();
      if (activityText.length > 0) {
        await expect(liveAnswer).not.toContainText(activityText);
      }
    }
    await expect(liveAnswer).not.toContainText('[retrieval_summary]');
    await expect(liveAnswer).not.toContainText('DSML');

    expect(errors).toEqual([]);
  });
});
