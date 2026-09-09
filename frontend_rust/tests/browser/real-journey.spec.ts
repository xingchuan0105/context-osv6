import {test, expect, type Page, type APIRequestContext} from '@playwright/test';
import {LIVE_API_BASE, obtainLiveJwt} from './live-auth';
import {seedNextAuth} from './auth-seed';
import {mkdirSync} from 'node:fs';

test.skip(process.env.LIVE_BACKEND !== '1', 'Real backend requires explicit opt-in');
const evidence = '/tmp/context-rust-real-journey';
test.beforeEach(async ({page}) => {
  mkdirSync(evidence, {recursive:true});
  await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, LIVE_API_BASE);
});
async function signIn(page:Page, request:APIRequestContext) {
  await seedNextAuth(page, await obtainLiveJwt(request));
}
async function newWorkspace(page:Page, request:APIRequestContext) {
  await signIn(page, request);
  await page.goto('/dashboard', {waitUntil:'networkidle'});
  await page.getByTestId('create-workspace-btn').click();
  const name = `UI acceptance ${Date.now()}`;
  await page.getByTestId('new-workspace-name').fill(name);
  await page.getByTestId('new-workspace-desc').fill('Synthetic data for local acceptance; safe to remove.');
  await page.getByTestId('submit-create-workspace').click();
  await expect(page).toHaveURL(/\/dashboard\/[0-9a-f-]+$/);
  await expect(page.locator('.app-context-title')).toHaveText(name);
  return page.url();
}

test('real UI login and Excel/PPT attachments enter only the current turn', async ({page, request}) => {
  await obtainLiveJwt(request);
  await page.goto('/login', {waitUntil:'networkidle'});
  await page.getByTestId('login-email').fill(process.env.E2E_TEST_USER_EMAIL || 'e2e-test@example.com');
  await page.getByTestId('login-password').fill(process.env.E2E_TEST_USER_PASSWORD || 'E2eTest123!');
  await page.getByTestId('login-submit').click();
  await expect(page).toHaveURL(/\/chat$/);
  await expect(page.getByTestId('scope-cap-rag')).toHaveCount(0);
  const indexRequests:string[] = [];
  page.on('request', req => { if (/\/documents|\/reindex|\/files(?:\/|$)/.test(new URL(req.url()).pathname)) indexRequests.push(req.url()); });
  await page.getByTestId('turn-attachment-input').setInputFiles(['../fixtures/live-office/inventory.xlsx', '../fixtures/live-office/delivery.pptx']);
  await expect(page.getByTestId('turn-attachment-item')).toHaveCount(2, {timeout:60000});
  await expect(page.getByTestId('turn-attachment-error')).toBeHidden();
  const sent = page.waitForRequest(req => req.method() === 'POST' && req.url().includes('/chat') && !!req.postDataJSON()?.attachments);
  await page.getByTestId('composer-input').fill('Read both attachments. What are the Cedar units and the delivery label?');
  await page.getByTestId('send-button').click();
  const body = (await sent).postDataJSON();
  expect(body.capabilities).toEqual([]);
  expect(body.attachments).toHaveLength(2);
  expect(JSON.stringify(body.attachments)).toContain('137');
  expect(JSON.stringify(body.attachments)).toContain('Violet Harbor');
  await expect(page.getByTestId('status-line')).toHaveText('已完成', {timeout:120000});
  await expect(page.getByTestId('live-answer')).toContainText('137');
  await expect(page.getByTestId('live-answer')).toContainText('Violet Harbor');
  await expect(page.getByTestId('turn-attachment-item')).toHaveCount(0);
  expect(indexRequests).toEqual([]);
  await page.screenshot({path:`${evidence}/office-answer.png`});
  const next = page.waitForRequest(req => req.method() === 'POST' && req.url().includes('/chat') && !!req.postDataJSON()?.query);
  await page.getByTestId('composer-input').fill('Reply with OK.');
  await page.getByTestId('send-button').click();
  expect((await next).postDataJSON().attachments ?? []).toEqual([]);
  await expect(page.getByTestId('status-line')).toHaveText('已完成', {timeout:120000});
});

test('real long answer grows before completion and survives history reload', async ({page, request}) => {
  await signIn(page, request);
  await page.goto('/chat', {waitUntil:'networkidle'});
  await page.getByTestId('composer-input').fill('Write a detailed 2000 word explanation of how rain forms.');
  const started = Date.now();
  const response = page.waitForResponse(res => res.request().method() === 'POST' && res.headers()['content-type']?.includes('text/event-stream'));
  await page.getByTestId('send-button').click();
  const stream = await response;
  expect(stream.status()).toBe(200);
  await expect(page.getByTestId('live-answer')).toContainText(/\S{8}/, {timeout:30000});
  const firstTextMs = Date.now()-started;
  await expect(page.getByTestId('stop-button')).toBeVisible();
  const firstLength = (await page.getByTestId('live-answer').innerText()).length;
  await expect.poll(async () => (await page.getByTestId('live-answer').innerText()).length, {timeout:20000}).toBeGreaterThan(firstLength+80);
  await expect(page.getByTestId('stop-button')).toBeVisible();
  await expect(page.getByTestId('status-line')).toHaveText('已完成', {timeout:150000});
  const answer = await page.getByTestId('live-answer').innerText();
  expect(answer.split(/\s+/).length).toBeGreaterThan(1000);
  const events = (await stream.text()).split('\n').filter(line => line.startsWith('data:')).map(line => JSON.parse(line.slice(5)));
  expect(events.filter(event => event.event === 'error')).toEqual([]);
  expect(events.some(event => event.event === 'done')).toBe(true);
  expect(events.filter(event => event.event === 'token').length).toBeGreaterThan(2);
  expect(answer).not.toMatch(/final_answer|DSML|<code language|skill_request/);
  console.log('Long answer evidence:', {firstTextMs, elapsedMs:Date.now()-started, words:answer.split(/\s+/).length,
    answerDeltas:events.filter(event => event.event === 'token').length});
  await page.screenshot({path:`${evidence}/long-answer.png`});
  const ending = answer.trim().slice(-100);
  await page.reload({waitUntil:'networkidle'});
  await expect(page.getByTestId('chat-canvas')).toContainText(ending);
});

test('real generation stops after visible answer text', async ({page, request}) => {
  await signIn(page, request);
  await page.goto('/chat', {waitUntil:'networkidle'});
  await page.getByTestId('composer-input').fill('Write a detailed 2000 word explanation of how rain forms.');
  await page.getByTestId('send-button').click();
  await expect(page.getByTestId('live-answer')).toContainText(/\S{8}/, {timeout:30000});
  await page.getByTestId('stop-button').click();
  await expect(page.getByTestId('stop-button')).toBeHidden();
  await expect(page.getByTestId('composer-input')).toBeEnabled();
  await expect(page.getByTestId('retry-button')).toBeVisible();
  const partial = await page.getByTestId('live-answer').innerText();
  expect(partial.trim().length).toBeGreaterThan(0);
  await expect(page.getByTestId('live-answer')).toHaveText(partial);
  await page.screenshot({path:`${evidence}/stopped-after-text.png`});
});

test('real in-flight generation can be stopped before the answer arrives', async ({page, request}) => {
  await signIn(page, request);
  await page.goto('/chat', {waitUntil:'networkidle'});
  await page.getByTestId('composer-input').fill('Write a detailed 2000 word explanation of how rain forms.');
  const response = page.waitForResponse(res => res.request().method() === 'POST' && res.headers()['content-type']?.includes('text/event-stream'));
  await page.getByTestId('send-button').click();
  expect((await response).status()).toBe(200);
  await page.getByTestId('stop-button').click();
  await expect(page.getByTestId('stop-button')).toBeHidden();
  await expect(page.getByTestId('composer-input')).toBeEnabled();
  await expect(page.getByTestId('retry-button')).toBeVisible();
  await page.screenshot({path:`${evidence}/stopped.png`});
});

test('real workspace note persists and uploaded source becomes searchable', async ({page, request}) => {
  const url = await newWorkspace(page, request);
  console.log('Acceptance workspace:', url);
  await page.getByTestId('workspace-open-notes').click();
  await page.getByTestId('btn-new-note').click();
  await page.getByTestId('note-title-input').fill('Acceptance note');
  await page.locator('.note-editor-host [contenteditable="true"]').fill('The note belongs to this workspace.');
  await page.getByTestId('submit-note-btn').click();
  await expect(page.getByTestId('workspace-note-item')).toContainText('Acceptance note');
  await page.reload({waitUntil:'networkidle'});
  await page.getByTestId('workspace-open-notes').click();
  await expect(page.getByTestId('workspace-note-item')).toContainText('Acceptance note');
  await page.getByTestId('workspace-panel-close').click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('workspace-quick-add').click();
  await (await chooser).setFiles({name:'cedar-reference.txt', mimeType:'text/plain', buffer:Buffer.from('Cedar equipment acceptance record. The inspection location is Amber Dock. The accepted unit count is 137. This is a synthetic test document.')});
  const doc = page.getByTestId('workspace-doc-item').filter({hasText:'cedar-reference.txt'});
  await expect(doc).toHaveAttribute('data-status','completed',{timeout:120000});
  await doc.getByTestId('doc-select').check();
  await page.getByTestId('workspace-panel-close').click();
  await page.getByTestId('composer-input').fill('According to the source, where was the Cedar equipment inspected?');
  await page.getByTestId('send-button').click();
  await expect(page.getByTestId('status-line')).toHaveText('已完成',{timeout:120000});
  await expect(page.getByTestId('live-answer')).toContainText('Amber Dock');
  await expect(page.getByTestId('workspace-session-item')).toHaveCount(1);
  await page.screenshot({path:`${evidence}/workspace-answer.png`});
  await page.reload({waitUntil:'networkidle'});
  await expect(page.getByTestId('workspace-session-item')).toHaveCount(1);
  await expect(page.getByTestId('chat-canvas')).toContainText('Amber Dock');
});

test('real share opens anonymously and revocation invalidates the link', async ({page, request, browser}) => {
  const workspace = await newWorkspace(page, request);
  console.log('Acceptance share workspace:', workspace);
  try {
  await page.goto(`${workspace}/share`, {waitUntil:'networkidle'});
  await page.getByTestId('create-share-btn').click();
  await expect(page.getByTestId('share-url')).toBeVisible();
  const share = await page.getByTestId('share-url').innerText();
  const visitor = await browser.newContext();
  await visitor.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, LIVE_API_BASE);
  const anonymous = await visitor.newPage();
  try {
    await anonymous.goto(share, {waitUntil:'networkidle'});
    await expect(anonymous.getByTestId('shared-kb-header')).toBeVisible();
    await expect(anonymous.getByTestId('session-list')).toHaveCount(0);
    await anonymous.screenshot({path:`${evidence}/public-share.png`});
  } finally {
    await page.getByTestId('revoke-share-btn').click();
    await expect(page.getByTestId('create-share-btn')).toBeVisible();
    await anonymous.reload({waitUntil:'networkidle'});
    await expect(anonymous.getByTestId('share-expired')).toBeVisible();
    await visitor.close();
  }
  } finally {
    // Revoking a token does not release the workspace's enabled-share slot.
    // Only this test's newly created workspace is restored to private.
    const id = new URL(workspace).pathname.split('/').at(-1);
    const restored = await request.put(`${LIVE_API_BASE}/api/v1/workspaces/${id}/share/settings`, {
      headers:{Authorization:`Bearer ${await obtainLiveJwt(request)}`},
      data:{access_level:'private'},
    });
    expect(restored.ok()).toBe(true);
  }
});
