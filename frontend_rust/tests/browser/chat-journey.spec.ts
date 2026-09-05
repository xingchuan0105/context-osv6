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
    } | null;
  }>;
}

test.describe('SSR smoke（Gate D）', () => {
  test('/chat 与 /chat/test-session 返回含表单语义的 SSR HTML', async ({ request }) => {
    for (const path of ['/chat', '/chat/test-session']) {
      const response = await request.get(`${WEB_BASE}${path}`);
      expect(response.status()).toBe(200);
      const html = await response.text();
      // 表单语义：label + textarea + 三个按钮
      expect(html).toContain('for="chat-composer-input"');
      expect(html).toContain('<textarea');
      expect(html).toContain('发送');
      expect(html).toContain('停止');
      expect(html).toContain('重试');
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
    await expect(page.getByTestId('session-item')).toHaveCount(1);
    await expect(page.getByTestId('session-item')).toContainText('夹具会话');
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

    // 验证普通未 hash 样式 (/style/chat-poc.css) 不被误设永久 immutable
    const unhashedCss = await request.get(`${WEB_BASE}/style/chat-poc.css`);
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

test.describe('会话文件（W2.4）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('上传后出现就绪文件，URL 落到新会话，发送可用', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/files`, '/chat', 'poc-test-token');
    await expect(page.getByTestId('session-file-attach')).toBeEnabled();
    const fileInput = page.getByTestId('session-file-input');
    await expect(fileInput).toHaveAttribute('data-listening', 'true', { timeout: 15_000 });

    await fileInput.setInputFiles({
      name: 'notes.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('hello'),
    });
    await fileInput.dispatchEvent('change');

    await expect(page).toHaveURL(/\/chat\/sess-file-1$/, { timeout: 15_000 });
    const item = page.getByTestId('session-file-item');
    await expect(item).toContainText('notes.txt', { timeout: 15_000 });
    await expect(page.getByTestId('session-file-status')).toHaveText('就绪', { timeout: 15_000 });
    await expect(page.getByTestId('session-file-blocked')).toHaveCount(0);

    await expect(page.getByTestId('scope-cap-rag')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId('scope-mode-line')).toContainText('知识库');

    await page.getByTestId('composer-input').fill('文件已就绪');
    await expect(page.getByTestId('send-button')).toBeEnabled();
    await page.getByTestId('send-button').click();
    await expect(page.getByTestId('status-line')).toHaveText('已完成', { timeout: 15_000 });
    const state = await fixtureState(request);
    expect(state.lastChatBody?.capabilities).toEqual(['rag']);
    expect(state.lastChatBody?.agent_type).toBe('rag');
    expect(errors).toEqual([]);
  });

  test('移除最后一份就绪文件后自动关掉知识库', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/files`, '/chat', 'poc-test-token');
    const fileInput = page.getByTestId('session-file-input');
    await expect(fileInput).toHaveAttribute('data-listening', 'true', { timeout: 15_000 });
    await fileInput.setInputFiles({
      name: 'notes.txt',
      mimeType: 'text/plain',
      buffer: Buffer.from('hello'),
    });
    await fileInput.dispatchEvent('change');
    await expect(page.getByTestId('session-file-status')).toHaveText('就绪', { timeout: 15_000 });
    await expect(page.getByTestId('scope-cap-rag')).toHaveAttribute('aria-pressed', 'true');

    await page.getByTestId('session-file-remove').click();
    await expect(page.getByTestId('session-file-item')).toHaveCount(0, { timeout: 15_000 });
    await expect(page.getByTestId('scope-cap-rag')).toHaveAttribute('aria-pressed', 'false');
    await expect(page.getByTestId('scope-mode-line')).toContainText('未添加会话文件');
    expect(errors).toEqual([]);
  });

  test('解析中的会话文件挡住发送', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/files-busy`, '/chat/sess-busy', 'poc-test-token');
    await expect(page.getByTestId('session-file-item')).toContainText('busy.pdf');
    await expect(page.getByTestId('session-file-status')).toHaveText('解析中');
    await expect(page.getByTestId('session-file-blocked')).toBeVisible();
    await page.getByTestId('composer-input').fill('还不能发');
    await expect(page.getByTestId('send-button')).toBeDisabled();
    expect(errors).toEqual([]);
  });
});

test.describe('能力标签（W2.5）', () => {
  test.beforeEach(async ({ request }) => {
    await request.post(`${FIXTURE_BASE}/admin/reset`);
  });

  test('默认发送仍是空 capabilities / chat', async ({ page, request }) => {
    const errors = collectPageErrors(page);
    await gotoChat(page, `${FIXTURE_BASE}/case/markdown`);
    await expect(page.getByTestId('scope-cap-rag')).toHaveAttribute('aria-pressed', 'false');
    await expect(page.getByTestId('scope-cap-search')).toHaveAttribute('aria-pressed', 'false');
    await expect(page.getByTestId('scope-mode-line')).toContainText('未添加会话文件');

    await page.getByTestId('scope-cap-rag').click();
    await expect(page.getByTestId('scope-cap-rag')).toHaveAttribute('aria-pressed', 'false');

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
    await expect(page.getByTestId('scope-mode-line')).toContainText('网络搜索');

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
