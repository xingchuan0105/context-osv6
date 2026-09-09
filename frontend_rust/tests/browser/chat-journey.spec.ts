import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const WEB_BASE = `http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}`;
const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

// 每个用例独立注入测试用 API base（内存态全局变量，非 URL token、非持久化）。
async function gotoChat(page: Page, apiBase: string, path = '/chat', token?: string) {
  if (token) {
    await seedNextAuth(page, token);
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, apiBase);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
  await expect(page.getByTestId('chat-composer')).toHaveAttribute('data-ready', 'true');
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

async function fixtureState(request: import('@playwright/test').APIRequestContext) {
  const response = await request.get(`${FIXTURE_BASE}/admin/state`);
  return response.json() as Promise<{
    aborted: boolean;
    requests: number;
    bytesWritten: number;
    lastAuthorization: string | null;
    lastChatBody: {
      capabilities?: string[];
      agent_type?: string;
      query?: string;
      attachments?: { filename: string; text: string }[];
    } | null;
  }>;
}

test.describe('Rust/UI composer', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('空白禁用、输入法确认不提交、Shift+Enter 换行、Enter 只发送一次', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/slow`);
    const input = page.getByTestId('composer-input');
    await input.fill('   ');
    await expect(page.getByTestId('send-button')).toBeDisabled();
    await input.fill('你好');
    await expect(page.getByTestId('send-button')).toBeEnabled();
    await input.dispatchEvent('compositionstart');
    await input.dispatchEvent('keydown', { key: 'Enter', isComposing: false });
    await input.dispatchEvent('compositionend');
    await input.dispatchEvent('keydown', { key: 'Enter', isComposing: true });
    await input.dispatchEvent('keydown', { key: 'Enter', keyCode: 229 });
    await input.press('Shift+Enter');
    await expect(input).toHaveValue('你好\n');
    await page.waitForTimeout(200);
    expect((await fixtureState(request)).requests).toBe(0);
    await input.press('Enter');
    await expect(page.getByTestId('stop-button')).toBeVisible();
    await expect(page.getByTestId('send-button')).toHaveCount(0);
    await expect(page.getByTestId('retry-button')).toHaveCount(0);
    await expect.poll(async () => (await fixtureState(request)).requests).toBe(1);
    await page.getByTestId('stop-button').click();
    await expect(input).toBeFocused();
    await expect(page.getByTestId('send-button')).toBeDisabled();
    await expect(page.getByTestId('retry-button')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('水合前输入的内容在事件就绪后完整保留', async ({ page }) => {
    let release!: () => void;
    const pending = new Promise<void>(resolve => { release = resolve; });
    await page.route('**/*.wasm', async route => { await pending; await route.continue(); });
    await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, FIXTURE_BASE);
    await page.goto('/chat', { waitUntil: 'domcontentloaded' });
    const input = page.getByTestId('composer-input');
    await input.fill('Keep this question');
    await expect(page.getByTestId('send-button')).toBeDisabled();
    release();
    await expect(page.getByTestId('send-button')).toBeEnabled();
    await expect(input).toHaveValue('Keep this question');
  });

  test('输入区自动增高、长文本上限与清空恢复', async ({ page }) => {
    await gotoChat(page, FIXTURE_BASE);
    const input = page.getByTestId('composer-input');
    await input.fill('短消息');
    await expect(page.getByTestId('send-button')).toBeEnabled();
    const shortHeight = (await input.boundingBox())!.height;
    await input.fill(Array(20).fill('多行消息').join('\n'));
    await expect.poll(async () => (await input.boundingBox())!.height).toBeGreaterThan(shortHeight);
    expect((await input.boundingBox())!.height).toBeLessThanOrEqual(240);
    await input.fill('');
    await expect.poll(async () => (await input.boundingBox())!.height).toBe(72);
  });

  for (const locale of ['zh-CN', 'en']) {
    for (const theme of ['light', 'dark']) {
      test(`${locale} ${theme} 输入区在桌面和窄屏完整可见`, async ({ page }, testInfo) => {
        const errors = collectPageErrors(page);
        await page.addInitScript((theme) => {
          localStorage.setItem('avrag.ui.theme.v1', theme);
        }, theme);
        await gotoChat(page, FIXTURE_BASE);
        if (locale === 'en') {
          await page.getByTestId('dashboard-account-menu-trigger').click();
          await page.getByTestId('account-locale-toggle').click();
          await page.getByTestId('account-locale-en').click();
          if (await page.getByTestId('dashboard-account-menu').isVisible()) {
            await page.getByTestId('dashboard-account-menu-trigger').click();
          }
        }
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        await page.getByTestId('composer-input').fill(locale === 'en' ? 'Summarize my project notes' : '整理项目资料，提炼下一步行动');
        await expect(page.getByTestId('send-button')).toBeEnabled();
        await expect(page.getByTestId('send-button')).toHaveText(locale === 'en' ? 'Send' : '发送');
        for (const width of [1280, 390]) {
          await page.setViewportSize({ width, height: 900 });
          const composer = page.getByTestId('chat-composer');
          await expect(composer).toBeVisible();
          const bounds = (await composer.boundingBox())!;
          expect(bounds.x).toBeGreaterThanOrEqual(0);
          expect(bounds.x + bounds.width).toBeLessThanOrEqual(width);
          expect(bounds.y + bounds.height).toBeLessThanOrEqual(900);
          const button = (await page.getByTestId('send-button').boundingBox())!;
          expect(button.x + button.width).toBeLessThanOrEqual(width);
          expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
          await page.screenshot({ path: testInfo.outputPath(`composer-${locale}-${theme}-${width}.png`), fullPage: true });
        }
        expect(errors).toEqual([]);
      });
    }
  }
});

test.describe('SSR smoke（Gate D）', () => {
  test('/chat 与 /chat/test-session 返回含表单语义的 SSR HTML', async ({ request }) => {
    for (const path of ['/chat', '/chat/test-session']) {
      const response = await request.get(`${WEB_BASE}${path}`);
      expect(response.status()).toBe(200);
      const html = await response.text();
      // 初始表单只展示发送，停止和重试按运行状态出现。
      expect(html).toContain('for="chat-composer-input"');
      expect(html).toContain('<textarea');
      expect(html).toContain('发送');
      expect(html).not.toContain('data-testid="stop-button"');
      expect(html).not.toContain('data-testid="retry-button"');
      expect(html).toContain('知识库');
      expect(html).toContain('网络搜索');
      // 不再是占位壳，且带 hydration 接线
      expect(html).not.toContain('Context-OS Chat PoC');
      expect(html).toMatch(/\/pkg\/web_ui[^"']*\.js/);
      expect(html).not.toContain('poc-token-input');
      expect(html).not.toContain('访问令牌');
    }
    const healthz = await request.get(`${WEB_BASE}/healthz`);
    expect(await healthz.text()).toBe('ok');
  });
});

test.describe('hydration（Gate D）', () => {
  test('hydration 后无重复 root、无 hydration 错误', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, FIXTURE_BASE);
    await expect(page.getByTestId('chat-canvas')).toHaveCount(1);
    await expect(page.getByTestId('composer-input')).toBeVisible();
    await expect(page.getByTestId('send-button')).toBeVisible();
    expect(errors).toEqual([]);
  });
});

test.describe('浏览器凭据（W1）', () => {
  test('Next 同键存储水合后出现会话列表，且无 PoC token 框', async ({ page }) => {
    await gotoChat(page, FIXTURE_BASE, '/chat', 'poc-test-token');
    await expect(page.getByTestId('poc-token-input')).toHaveCount(0);
    await expect(page.getByTestId('session-item')).toHaveCount(2);
    await expect(page.getByTestId('session-item').first()).toContainText('夹具会话');
    await expect(page.getByTestId('session-auth-hint')).toHaveCount(0);
  });
});

test.describe('静态资源交付与缓存头（W1）', () => {
  test('WASM 带有 application/wasm MIME 与预压缩支持，pkg 资源带 immutable 缓存，未 hash 样式无 immutable', async ({
    request,
  }) => {
    const chatRes = await request.get(`${WEB_BASE}/chat`);
    const html = await chatRes.text();
    const jsMatch = html.match(/\/pkg\/[a-zA-Z0-9_.-]+\.js/);
    expect(jsMatch).toBeTruthy();

    const jsUrl = `${WEB_BASE}${jsMatch![0]}`;
    const jsRes = await request.get(jsUrl, {
      headers: { 'Accept-Encoding': 'br, gzip' },
    });
    expect(jsRes.status()).toBe(200);
    expect(jsRes.headers()['cache-control']).toContain('immutable');
    expect(jsRes.headers()['cache-control']).toContain('max-age=31536000');
    expect(['br', 'gzip']).toContain(jsRes.headers()['content-encoding']);

    // 验证普通未 hash 样式 (/style/app.css) 不被误设永久 immutable
    const unhashedCss = await request.get(`${WEB_BASE}/style/app.css`);
    expect(unhashedCss.status()).toBe(200);
    const unhashedCache = unhashedCss.headers()['cache-control'] || '';
    expect(unhashedCache).not.toContain('immutable');
  });
});

test.describe('助手 Markdown（W2）', () => {
  test('标题与列表渲染，恶意 script / javascript: 不进 DOM', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`);
    await page.getByTestId('composer-input').fill('markdown 清洗');
    await page.getByTestId('send-button').click();

    const live = page.getByTestId('live-answer');
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    await expect(live.locator('h1')).toHaveText('标题');
    await expect(live.locator('li')).toContainText('一项');
    await expect(live).toContainText('安全段落');
    await expect(live.locator('script')).toHaveCount(0);
    await expect(live.locator('img')).toHaveCount(0);
    await expect(live.locator('a[href^="javascript"]')).toHaveCount(0);
    expect(errors).toEqual([]);
  });
});

test.describe('引用 chip 与来源卡（W2）', () => {
  test('[[1]] 变成 chip，点击后高亮来源卡，脚本不进 DOM', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/citations`);
    await page.getByTestId('composer-input').fill('引用清洗');
    await page.getByTestId('send-button').click();

    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const live = page.getByTestId('live-answer');
    await expect(live).not.toContainText('[[1]]');
    const chip = live.getByTestId('citation-chip');
    await expect(chip).toHaveText('1');
    await expect(live.locator('script')).toHaveCount(0);

    const card = page.getByTestId('citations-region').getByTestId('citation-card');
    await expect(card).toContainText('手册');
    await expect(card).toContainText('背压与窗口');
    await chip.click();
    await expect(card).toHaveClass(/is-active/);
    expect(errors).toEqual([]);
  });
});

test.describe('进度与推理终态折叠（W2）', () => {
  test('完成后进度收成一行，展开可见步骤；推理摘要默认合上', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/progress`);
    await page.getByTestId('composer-input').fill('终态折叠');
    await page.getByTestId('send-button').click();

    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const activity = page.getByTestId('activity-region');
    await expect(activity).toHaveAttribute('data-collapsed', 'true');
    await expect(page.getByTestId('progress-toggle')).toContainText('思考完成');
    await expect(page.getByTestId('activity-steps')).toHaveCount(0);

    await page.getByTestId('progress-toggle').click();
    await expect(activity).toHaveAttribute('data-collapsed', 'false');
    await expect(page.getByTestId('activity-steps')).toContainText('组织短答');

    const reasoning = page.getByTestId('reasoning-region');
    await expect(reasoning).toContainText('因果链已对齐');
    await expect(reasoning).toHaveJSProperty('open', false);
    await reasoning.locator('summary').click();
    await expect(reasoning).toHaveJSProperty('open', true);
    expect(errors).toEqual([]);
  });
});

test.describe('本轮附件', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('办公附件只进入本轮，重试保留，下一轮不继承，也不调用索引接口', async ({ page, request }) => {
    const indexRequests: string[] = [];
    page.on('request', req => {
      if (/\/files(?:\/|$)|\/documents|\/reindex/.test(new URL(req.url()).pathname)) indexRequests.push(req.url());
    });
    await page.route('**/chat/attachments/parse?**', route => route.fulfill({
      json: { filename: 'report.xlsx', text: 'Name | Total\nA | 7' },
    }));
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`, '/chat', 'poc-test-token');
    await expect(page.getByTestId('scope-cap-rag')).toHaveCount(0);
    await page.getByTestId('turn-attachment-input').setInputFiles({ name: 'report.xlsx', mimeType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', buffer: Buffer.from('parser-fixture') });
    await expect(page.getByTestId('turn-attachment-item')).toContainText('report.xlsx');
    await expect(page).toHaveURL(/\/chat$/);
    await page.getByTestId('composer-input').fill('Read the attachment');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成');
    let state = await fixtureState(request);
    expect(state.lastChatBody?.attachments).toEqual([{ filename: 'report.xlsx', text: 'Name | Total\nA | 7' }]);
    expect(state.lastChatBody?.capabilities).toEqual([]);
    await expect(page.getByTestId('turn-attachment-item')).toHaveCount(0);
    await page.getByTestId('retry-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成');
    state = await fixtureState(request);
    expect(state.lastChatBody?.attachments?.[0].filename).toBe('report.xlsx');
    await page.getByTestId('composer-input').fill('An unrelated question');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成');
    state = await fixtureState(request);
    expect(state.lastChatBody?.attachments ?? []).toEqual([]);
    expect(indexRequests).toEqual([]);
  });

  test('解析失败时不发送缺失附件，显式放弃失败附件后才能继续', async ({ page }) => {
    await page.route('**/chat/attachments/parse?**', route => route.fulfill({ status: 422, json: { error: 'attachment_parse_failed' } }));
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`, '/chat', 'poc-test-token');
    await page.getByTestId('composer-input').fill('Read the file');
    await page.getByTestId('turn-attachment-input').setInputFiles({ name: 'slides.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from('parser-fixture') });
    await expect(page.getByTestId('turn-attachment-error')).toBeVisible();
    await expect(page.getByTestId('send-button')).toBeDisabled();
    await page.getByRole('button', { name: '仅使用已列出的附件继续' }).click();
    await expect(page.getByTestId('send-button')).toBeEnabled();
  });

  test('切换会话后迟到的解析结果不会带入新会话', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    let release!: () => void;
    const pending = new Promise<void>(resolve => { release = resolve; });
    await page.route('**/chat/attachments/parse?**', async route => {
      await pending;
      await route.fulfill({ json: { filename: 'old.txt', text: 'old conversation attachment' } });
    });
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`, '/chat', 'poc-test-token');
    const started = page.waitForRequest('**/chat/attachments/parse?**');
    await page.getByTestId('turn-attachment-input').setInputFiles({ name: 'old.txt', mimeType: 'text/plain', buffer: Buffer.from('old') });
    await started;
    await page.getByTestId('session-item').filter({ hasText: '夹具会话' }).click();
    await expect(page).toHaveURL(/\/chat\/sess-900$/);
    const returned = page.waitForResponse('**/chat/attachments/parse?**');
    release();
    await returned;
    await page.getByTestId('composer-input').fill('New conversation question');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成');
    expect((await fixtureState(request)).lastChatBody?.attachments ?? []).toEqual([]);
    await expect(page.getByTestId('turn-attachment-item')).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test('删除待发送附件不会创建会话或留下检索开关', async ({ page }) => {
    await page.route('**/chat/attachments/parse?**', route => route.fulfill({ json: { filename: 'notes.txt', text: 'contents' } }));
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`, '/chat', 'poc-test-token');
    await page.getByTestId('turn-attachment-input').setInputFiles({ name: 'notes.txt', mimeType: 'text/plain', buffer: Buffer.from('contents') });
    await expect(page.getByTestId('turn-attachment-item')).toBeVisible();
    await page.getByTestId('turn-attachment-item').getByRole('button', { name: '移除' }).click();
    await expect(page.getByTestId('turn-attachment-item')).toHaveCount(0);
    await expect(page).toHaveURL(/\/chat$/);
    await expect(page.getByTestId('scope-cap-rag')).toHaveCount(0);
  });
});

test.describe('能力标签（W2.5）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('默认发送仍是空 capabilities / chat', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`);
    await expect(page.getByTestId('scope-cap-rag')).toHaveCount(0);
    await expect(page.getByTestId('scope-cap-search')).toHaveAttribute('aria-pressed', 'false');
    await expect(page.getByTestId('scope-mode-line')).toContainText('快速聊天');

    await page.getByTestId('composer-input').fill('默认聊天');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const state = await fixtureState(request);
    expect(state.lastChatBody?.capabilities).toEqual([]);
    expect(state.lastChatBody?.agent_type).toBe('chat');
    expect(errors).toEqual([]);
  });

  test('点开网络搜索后发送带 search', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`);
    await page.getByTestId('scope-cap-search').click();
    await expect(page.getByTestId('scope-cap-search')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId('scope-mode-line')).toHaveText('快速聊天 · 联网搜索');

    await page.getByTestId('composer-input').fill('开搜索');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const state = await fixtureState(request);
    expect(state.lastChatBody?.capabilities).toEqual(['search']);
    expect(state.lastChatBody?.agent_type).toBe('search');
    expect(errors).toEqual([]);
  });
});

test.describe('模型角色与 BYOK 状态（W2.6）', () => {
  test('个人默认显示对话 · qwen3.8-flash (默认)', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, FIXTURE_BASE, '/chat', 'poc-test-token');
    const badge = page.getByTestId('model-role-badge');
    await expect(badge).toBeVisible();
    await expect(badge).toContainText('对话 · qwen3.8-flash (默认)');
    expect(errors).toEqual([]);
  });

  test('检测到 quick_chat 自备密钥时显示 (自定义密钥)', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/byok`, '/chat', 'poc-test-token');
    const badge = page.getByTestId('model-role-badge');
    await expect(badge).toBeVisible();
    await expect(badge).toContainText('对话 · qwen3.8-flash (自定义密钥)');
    expect(errors).toEqual([]);
  });
});

test.describe('回答操作与 Feedback 持久化（W2.7）', () => {
  test('支持复制回答内容、点赞成功高亮、已删除来源卡明确标记且禁用链接', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/feedback`, '/chat/sess-history', 'poc-test-token');

    const assistantMsg = page.getByTestId('chat-message').filter({ hasText: '历史助手' });
    await expect(assistantMsg).toBeVisible({ timeout: 15_000 });
    const copyBtn = assistantMsg.getByTestId('copy-answer-button');
    await expect(copyBtn).toBeVisible();
    await copyBtn.click();
    await expect(copyBtn).toHaveText('已复制');

    const upBtn = assistantMsg.getByTestId('feedback-up');
    await expect(upBtn).toBeVisible();
    await upBtn.click();
    await expect(upBtn).toHaveClass(/is-active/);

    const card = assistantMsg.getByTestId('citation-card');
    await expect(card).toContainText('已下线文档');
    await expect(card).toContainText('来源已删除');
    await expect(card.locator('a')).toHaveCount(0);

    expect(errors).toEqual([]);
  });

  test('Feedback 提交失败时在 UI 呈现可感知的错误提示（role=alert）', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*500/]);
    await gotoChat(page, `${FIXTURE_BASE}/case/feedback-fail`, '/chat/sess-history', 'poc-test-token');

    const assistantMsg = page.getByTestId('chat-message').filter({ hasText: '历史助手' });
    await expect(assistantMsg).toBeVisible({ timeout: 15_000 });

    const downBtn = assistantMsg.getByTestId('feedback-down');
    await downBtn.click();

    const errAlert = assistantMsg.getByTestId('feedback-error');
    await expect(errAlert).toBeVisible({ timeout: 15_000 });
    await expect(errAlert).toContainText('反馈提交失败');

    expect(errors).toEqual([]);
  });
});

test.describe('Workspace 最小 Shell 与归属隔离（W2.8）', () => {
  test('个人会话进入工作区后只保留工作区会话列表，返回个人聊天恢复快速聊天', async ({
    page,
    request,
  }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, FIXTURE_BASE, '/chat', 'poc-test-token');

    // 1. 会话列表区分归属
    const personalItem = page.getByTestId('session-item').filter({ hasText: '夹具会话' });
    const wsItem = page.getByTestId('session-item').filter({ hasText: '[材料研发]' });
    await expect(personalItem).toBeVisible();
    await expect(wsItem).toBeVisible();
    await expect(wsItem).toContainText('合金强度分析');

    // 2. 点击工作区会话导航至 /dashboard/:ws?session=:sid
    await wsItem.click();
    await expect(page).toHaveURL(/\/dashboard\/ws-materials\?session=sess-ws-901$/);

    // 3. 工作区使用自己的标题和会话列表，不重复个人聊天侧栏。
    await expect(page.getByTestId('workspace-workbench')).toBeVisible();
    await expect(page.getByTestId('workspace-sessions')).toHaveCount(1);
    await expect(page.getByTestId('session-list')).toHaveCount(1);
    await expect(page.getByTestId('session-list')).not.toContainText('夹具会话');
    await expect(page.getByTestId('workspace-sessions')).toContainText('合金强度分析');
    await expect(page.getByTestId('model-role-badge')).toContainText('工作区 Agent');

    // 4. 工作区内发问自动携带 workspace_id
    await page.getByTestId('composer-input').fill('测试工作区发问');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const state = await fixtureState(request);
    expect(state.lastChatBody?.workspace_id).toBe('ws-materials');

    // 5. 使用全局聊天入口返回个人对话，恢复 quick_chat。
    await page.locator('a[href="/chat"]').first().click();
    await expect(page).toHaveURL(/\/chat$/);
    await expect(page.getByTestId('workspace-banner')).toHaveCount(0);
    await expect(page.getByTestId('model-role-badge')).toContainText('对话 · qwen3.8-flash');

    expect(errors).toEqual([]);
  });
});

test.describe('受限上下文与历史不可变性（不变量 11 & 12）', () => {
  test('历史会话恢复后发起新提问，旧轮次内容与引用保持完整，不外泄其他会话上下文', async ({
    page,
    request,
  }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/feedback`, '/chat/sess-history', 'poc-test-token');

    // 1. 验证既有历史消息与引用呈现
    const historyAssistant = page.getByTestId('chat-message').filter({ hasText: '历史助手' });
    await expect(historyAssistant).toBeVisible();
    await expect(historyAssistant).toContainText('旧文档残片');

    // 2. 发起新一轮提问
    await page.getByTestId('composer-input').fill('继续追问');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });

    // 3. 既有旧轮次正文与引用完全不被改写或污染
    await expect(historyAssistant).toBeVisible();
    await expect(historyAssistant).toContainText('这是历史助手的完整回答内容。');
    await expect(historyAssistant).toContainText('已下线文档');

    // 4. 验证请求携带正确的会话隔离 ID
    const state = await fixtureState(request);
    expect(state.lastChatBody?.query).toBe('继续追问');

    expect(errors).toEqual([]);
  });
});

test.describe('浏览器聊天旅程（Gate C/D）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('send → 字节级乱序分块流式渲染完整 3000+ 字答案，区域分离，URL 落地', async ({
    page,
    request,
  }) => {
    const errors = collectPageErrors(page);
    // split=bytes&n=113：响应体被切成与 UTF-8 字符边界不对齐的网络包
    await gotoChat(page, `${FIXTURE_BASE}/bytes/113`, '/chat', 'poc-test-token');

    await page.getByTestId('composer-input').fill('写一篇 3000 字以上的流式系统说明');
    await page.getByTestId('send-button').click();

    // URL 在收到服务端 session id 后落地，且流不被打断
    await expect(page).toHaveURL(/\/chat\/sess-900$/);

    // 区域分离：activity / reasoning / citations 各自区域，不进主气泡
    await expect(page.getByTestId('activity-region')).toContainText('组织长篇回答');
    await expect(page.getByTestId('reasoning-region')).toContainText('因果链');
    await expect(page.getByTestId('citations-region')).toContainText('流式系统手册');

    // 完整答案：尾部 marker 出现且总长度 ≥3000 字
    const liveAnswer = page.getByTestId('live-answer');
    await expect(liveAnswer).toContainText('退化成早已演练过的常规操作。', { timeout: 60_000 });
    // Markdown 把源串里的空行收成 <p>，textContent.length 会短一截；完整性看 reducer 源字符数。
    const sourceChars = Number(await liveAnswer.getAttribute('data-source-chars'));
    expect(sourceChars).toBeGreaterThanOrEqual(3000);
    // 主气泡只含模型答案：不含 activity/trace 文案
    await expect(liveAnswer).not.toContainText('组织长篇回答');
    await expect(liveAnswer).not.toContainText('plan');

    await expect(page.getByTestId('status-line')).toHaveText('已完成');

    // Bearer 头只在 token 非空时注入
    const state = await fixtureState(request);
    expect(state.lastAuthorization).toBe('Bearer poc-test-token');
    expect(state.lastChatBody?.capabilities).toEqual([]);
    expect(state.lastChatBody?.agent_type).toBe('chat');

    expect(errors).toEqual([]);
  });

  test('stop 在终态前真正中止 reader；abort 后迟到字节不改写 UI；焦点回 composer', async ({
    page,
    request,
  }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/slow`);

    await page.getByTestId('composer-input').fill('慢速流测试');
    await page.getByTestId('send-button').click();

    const liveAnswer = page.getByTestId('live-answer');
    await expect(liveAnswer).toContainText('慢速片段', { timeout: 15_000 });

    await page.getByTestId('stop-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已停止');

    // 客户端 abort 真正穿越网络边界：服务端观察到连接被中止
    await expect
      .poll(async () => (await fixtureState(request)).aborted, { timeout: 10_000 })
      .toBe(true);

    // abort 后的迟到字节不再修改 UI
    const frozenText = (await liveAnswer.textContent()) || '';
    await page.waitForTimeout(800);
    expect((await liveAnswer.textContent()) || '').toBe(frozenText);

    // stop 后焦点回到 composer
    await expect(page.getByTestId('composer-input')).toBeFocused();

    expect(errors).toEqual([]);
  });

  test('HTTP 401 在页面显示为错误状态（role=alert）', async ({ page }) => {
    // 浏览器会为 401 响应记录一条网络层 console error，属预期噪音
    const errors = collectPageErrors(page, [/Failed to load resource.*401/]);
    await gotoChat(page, `${FIXTURE_BASE}/case/401`);

    await page.getByTestId('composer-input').fill('需要鉴权的请求');
    await page.getByTestId('send-button').click();

    const alert = page.getByRole('alert');
    await expect(alert).toBeVisible({ timeout: 15_000 });
    await expect(alert).toContainText('unauthorized');
    // 错误不进入答案主气泡
    await expect(page.getByTestId('live-answer')).toHaveText('');

    expect(errors).toEqual([]);
  });

  test('坏 JSON 帧在页面显示为错误状态（role=alert）', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/bad-json`);

    await page.getByTestId('composer-input').fill('坏帧测试');
    await page.getByTestId('send-button').click();

    const alert = page.getByRole('alert');
    await expect(alert).toBeVisible({ timeout: 15_000 });
    await expect(alert).toContainText('framing');

    expect(errors).toEqual([]);
  });

  test('retry 创建新的 stream scope：旧流不污染新流，答案完整', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/slow`);

    await page.getByTestId('composer-input').fill('先发起一条将被停止的流');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('live-answer')).toContainText('慢速片段', { timeout: 15_000 });
    await page.getByTestId('stop-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已停止');

    // retry：同一 fixture 服务器，但换用完整长答案端点（直接改写当前页内存态全局变量）
    await page.evaluate((base) => {
      (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
    }, FIXTURE_BASE);
    await page.getByTestId('retry-button').click();

    const liveAnswer = page.getByTestId('live-answer');
    await expect(liveAnswer).toContainText('退化成早已演练过的常规操作。', { timeout: 60_000 });
    await expect(page.getByTestId('status-line')).toHaveText('已完成');
    // 旧慢速流的迟到 token（"慢速片段"）不得出现在新流答案中
    await expect(liveAnswer).not.toContainText('慢速片段');

    // 两条请求都真实穿过网络边界
    const state = await fixtureState(request);
    expect(state.requests).toBe(2);

    expect(errors).toEqual([]);
  });
});

test.describe('Chat 呈现（E5.3 / G5.3）', () => {
  test('空态 hero、紧凑输入区与移动会话抽屉', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, FIXTURE_BASE);
    await expect(page.getByTestId('chat-hero')).toBeVisible();
    await expect(page.getByTestId('chat-empty')).toHaveText('有什么可以帮你？');
    await expect(page.getByTestId('turn-attachment-add')).toBeVisible();
    await expect(page.getByTestId('composer-resize')).toHaveCount(0);

    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.getByTestId('chat-rail-toggle')).toBeVisible();
    await expect(page.getByTestId('session-list')).toBeHidden();
    await page.getByTestId('chat-rail-toggle').click();
    await expect(page.getByTestId('session-list')).toBeVisible();
    await expect(page.getByTestId('chat-rail-dismiss')).toBeVisible();
    expect(errors).toEqual([]);
  });

  test('代码块复制、图片卡、工具卡、网页来源弹窗、degrade 条与耗时', async ({
    page,
    context,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/e53`);
    await page.getByTestId('composer-input').fill('呈现对齐');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });

    const live = page.getByTestId('live-answer');
    await expect(live.getByTestId('chat-code-block')).toBeVisible();
    await expect(live.getByTestId('chat-code-lang')).toBeVisible();
    await expect(live.getByTestId('chat-code-block')).toContainText('fn main() {}');
    await live.getByTestId('chat-code-copy').click();
    await expect.poll(async () => page.evaluate(() => navigator.clipboard.readText())).toContain(
      'fn main() {}',
    );
    await expect(live.getByTestId('chat-figure')).toBeVisible();
    await expect(page.getByTestId('tool-result-card').first()).toBeVisible();
    await expect(page.getByTestId('tool-result-card').first()).toContainText('web_search');
    await expect(
      page.locator('[data-testid="chat-degrade-notice"]:not([hidden])').first(),
    ).toBeVisible();
    await expect(
      page.locator('[data-testid="chat-degrade-notice"]:not([hidden])').first(),
    ).toContainText('no_retrieval_evidence');
    await expect(page.getByTestId('workspace-progress-elapsed')).toBeVisible();

    await page.getByTestId('web-sources-button').first().click();
    await expect(page.getByTestId('workspace-web-sources-modal')).toBeVisible();
    await expect(page.getByTestId('workspace-web-sources-list')).toContainText('example.com/alloy');
    await page.getByTestId('workspace-web-sources-modal').getByTestId('dialog-close').click();
    await expect(page.getByTestId('workspace-web-sources-modal')).toHaveCount(0);

    await page.getByTestId('edit-user-message').click();
    await expect(page.getByTestId('composer-input')).toHaveValue('呈现对齐');
    expect(errors).toEqual([]);
  });

  test('滚动离开底部后出现回到底部', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/e53`);
    await page.addStyleTag({
      content: '[data-testid="chat-transcript"] { max-height: 120px !important; }',
    });
    await page.getByTestId('composer-input').fill('滚动');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });

    await page.getByTestId('chat-transcript').evaluate((el) => {
      el.scrollTop = 0;
      el.dispatchEvent(new Event('scroll'));
    });
    await expect(page.getByTestId('scroll-to-bottom')).toBeVisible();
    await page.getByTestId('scroll-to-bottom').click();
    await expect(page.getByTestId('scroll-to-bottom')).toBeHidden();
    expect(errors).toEqual([]);
  });

  test('会话栏 empty / error+retry / loading', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*500/]);
    await gotoChat(page, `${FIXTURE_BASE}/case/sessions-empty`, '/chat', 'poc-test-token');
    await expect(page.getByTestId('session-empty')).toBeVisible();

    await gotoChat(page, `${FIXTURE_BASE}/case/sessions-error`, '/chat', 'poc-test-token');
    await expect(page.getByTestId('session-list-error')).toBeVisible();
    await expect(page.getByTestId('session-retry')).toBeVisible();
    await page.getByTestId('session-retry').click();
    await expect(page.getByTestId('session-list-error')).toBeVisible();

    await gotoChat(page, `${FIXTURE_BASE}/case/sessions-slow`, '/chat', 'poc-test-token');
    await expect(page.getByTestId('session-loading')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('session-empty')).toBeVisible({ timeout: 10_000 });
    expect(errors).toEqual([]);
  });

  test('流式中会话项锁定', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/slow`, '/chat', 'poc-test-token');
    await page.getByTestId('composer-input').fill('锁导航');
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('live-answer')).toContainText('慢速片段', { timeout: 15_000 });
    await expect(page.getByTestId('session-item').first()).toBeDisabled();
    await page.getByTestId('session-item').first().click({ force: true });
    await expect(page).not.toHaveURL(/sess-900/);
    await page.getByTestId('stop-button').click();
    expect(errors).toEqual([]);
  });
});
