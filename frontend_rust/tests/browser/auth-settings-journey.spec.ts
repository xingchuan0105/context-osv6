import { test, expect, type Page } from '@playwright/test';

const WEB_BASE = `http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}`;
const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

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

async function gotoPage(page: Page, path: string) {
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('账号登录与凭据持久化（E3.1）', () => {
  test('登录成功后写入凭据并根据 next 参数回跳目标页', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoPage(page, '/login?next=/settings');

    await page.getByTestId('login-email').fill('alice@example.com');
    await page.getByTestId('login-password').fill('correct-password');
    await page.getByTestId('login-submit').click();

    // 成功后自动回跳到 /settings
    await expect(page).toHaveURL(/\/settings$/);
    await expect(page.getByTestId('settings-page')).toBeVisible();

    expect(errors).toEqual([]);
  });

  test('密码错误时在页面呈现清晰的错误警示（role=alert）', async ({ page }) => {
    const errors = collectPageErrors(page, [/Failed to load resource.*401/]);
    await gotoPage(page, '/login');

    await page.getByTestId('login-email').fill('alice@example.com');
    await page.getByTestId('login-password').fill('wrong');
    await page.getByTestId('login-submit').click();

    const alert = page.getByTestId('login-error');
    await expect(alert).toBeVisible();
    await expect(alert).toContainText('密码错误');

    expect(errors).toEqual([]);
  });
});

test.describe('账号注册流程（E3.1）', () => {
  test('未勾选条款时阻止提交，完整填写后注册成功并进入聊天', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoPage(page, '/register');

    await page.getByTestId('register-name').fill('Bob');
    await page.getByTestId('register-email').fill('bob@example.com');
    await page.getByTestId('register-password').fill('secret1234');
    await page.getByTestId('register-confirm-password').fill('secret1234');

    // 未同意条款前点击
    await page.getByTestId('register-submit').click();
    const alert = page.getByTestId('register-error');
    await expect(alert).toBeVisible();
    await expect(alert).toContainText('请阅读并同意');

    // 勾选条款并提交
    await page.getByTestId('consent-checkbox').check();
    await page.getByTestId('register-submit').click();

    await expect(page).toHaveURL(/\/chat$/);
    await expect(page.getByTestId('chat-canvas')).toBeVisible();

    expect(errors).toEqual([]);
  });
});

test.describe('密码重置流程（E3.1）', () => {
  test('申请验证码 → 核验 → 提交新密码完整闭环', async ({ page }) => {
    const errors = collectPageErrors(page);

    // 1. 发送验证码
    await gotoPage(page, '/reset-password');
    await page.getByTestId('reset-email').fill('charlie@example.com');
    await page.getByTestId('reset-request-submit').click();

    // 2. 核验验证码
    await expect(page).toHaveURL(/\/reset-password\/verify\?email=/);
    await page.getByTestId('reset-code').fill('123456');
    await page.getByTestId('reset-verify-submit').click();

    // 3. 设置新密码
    await expect(page).toHaveURL(/\/reset-password\/confirm\?ticket=/);
    await page.getByTestId('new-password').fill('newSecret123');
    await page.getByTestId('confirm-new-password').fill('newSecret123');
    await page.getByTestId('reset-confirm-submit').click();

    const success = page.getByTestId('reset-success');
    await expect(success).toBeVisible();
    await expect(success).toContainText('密码修改成功');

    expect(errors).toEqual([]);
  });
});

test.describe('设置页与自备密钥 BYOK 配置闭环（E3.1）', () => {
  test('配置 Quick Chat 自备密钥后状态切换为已配置，可成功移除，登出清理凭据', async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    // 先登录以建立 token 上下文
    await gotoPage(page, '/login?next=/settings');
    await page.getByTestId('login-email').fill('dave@example.com');
    await page.getByTestId('login-password').fill('pass123456');
    await page.getByTestId('login-submit').click();

    await expect(page).toHaveURL(/\/settings$/);
    await expect(page.getByTestId('providers-panel')).toBeVisible();

    // 1. 录入 Quick Chat 自备 API Key
    const input = page.getByTestId('input-quick_chat');
    await expect(input).toBeVisible();
    await input.fill('sk-bailian-custom-key-xyz');
    await page.getByTestId('save-quick_chat').click();

    // 2. 状态变为已配置，出现移除按钮
    const status = page.getByTestId('status-quick_chat');
    await expect(status).toBeVisible();
    await expect(status).toContainText('已配置 (自备密钥)');
    const revokeBtn = page.getByTestId('revoke-quick_chat');
    await expect(revokeBtn).toBeVisible();

    // 3. 移除密钥
    await revokeBtn.click();
    await expect(page.getByTestId('input-quick_chat')).toBeVisible();

    // 4. 退出登录
    await page.getByTestId('settings-logout').click();
    await expect(page).toHaveURL(/\/login$/);

    expect(errors).toEqual([]);
  });
});
