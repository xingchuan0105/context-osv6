import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';
import { LIVE_API_BASE, obtainLiveJwt } from './live-auth';

const LIVE_ENABLED = process.env.LIVE_BACKEND === '1';
const API_BASE = LIVE_API_BASE;

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

  test('Next 同键存储走一轮真实 Quick Chat', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    const token = await obtainLiveJwt(request);
    await seedNextAuth(page, token);
    await gotoChat(page, API_BASE);

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
