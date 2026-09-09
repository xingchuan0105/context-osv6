import {test, expect} from '@playwright/test';
import {seedNextAuth} from './auth-seed';

test('successful REST JSON larger than the error-body limit remains complete', async ({page}) => {
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(() => { (window as any).__POC_CHAT_API_BASE__ = 'http://127.0.0.1:3201'; });
  const sessions = Array.from({length:40}, (_, i) => ({
    id:`large-${i}`, owner_user_id:'fixture-user', scope_kind:'personal',
    title:`历史会话 ${i} — ${'完整保留'.repeat(20)}`, agent_type:'chat', model_role:'quick_chat',
    created_at:'2026-09-09T00:00:00Z', updated_at:'2026-09-09T00:00:00Z',
  }));
  expect(JSON.stringify({sessions}).length).toBeGreaterThan(4096);
  await page.route('**/api/v1/chat/sessions', route => route.fulfill({json:{sessions}}));
  await page.goto('/chat', {waitUntil:'networkidle'});
  await expect(page.getByRole('button', {name:sessions[39].title, exact:true})).toBeAttached();
  await expect(page.getByTestId('session-list')).not.toContainText('加载会话失败');
});

test('creating a public share sends the explicit read-only JSON contract', async ({page}) => {
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(() => { (window as any).__POC_CHAT_API_BASE__ = 'http://127.0.0.1:3201'; });
  await page.route('**/share/settings', route => route.fulfill({json:{share_token:'', access_level:'none', allow_download:false}}));
  let body:unknown;
  let contentType = '';
  await page.route('**/api/v1/workspaces/ws-materials/share', route => {
    body = route.request().postDataJSON();
    contentType = route.request().headers()['content-type'];
    return route.fulfill({json:{share_token:'read-only-test'}});
  });
  await page.goto('/dashboard/ws-materials/share', {waitUntil:'networkidle'});
  await page.getByTestId('create-share-btn').click();
  await expect(page.getByTestId('share-url')).toContainText('read-only-test');
  expect(body).toEqual({role:'viewer'});
  expect(contentType).toContain('application/json');
});
