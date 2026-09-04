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
    const answerLength = await liveAnswer.evaluate((el) => (el.textContent || '').length);
    expect(answerLength).toBeGreaterThanOrEqual(3000);
    // 主气泡只含模型答案：不含 activity/trace 文案
    await expect(liveAnswer).not.toContainText('组织长篇回答');
    await expect(liveAnswer).not.toContainText('plan');

    await expect(page.getByTestId('status-line')).toHaveText('已完成');

    // Bearer 头只在 token 非空时注入
    const state = await fixtureState(request);
    expect(state.lastAuthorization).toBe('Bearer poc-test-token');

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
