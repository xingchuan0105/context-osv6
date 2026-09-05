import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

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

async function gotoBilling(page: Page, path: string, token?: string) {
  if (token) {
    await seedNextAuth(page, token);
  }
  await page.addInitScript((base) => {
    (window as unknown as { __POC_CHAT_API_BASE__: string }).__POC_CHAT_API_BASE__ = base;
  }, FIXTURE_BASE);
  await page.goto(path, { waitUntil: 'domcontentloaded' });
}

test.describe('套餐定价与按需充值（E3.4）', () => {
  test('展示免费与 Pro 套餐，读取钱包余额，支持选择金额发起充值结账', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoBilling(page, '/pricing', 'poc-test-token');

    // 1. 套餐卡片展示
    await expect(page.getByTestId('pricing-page')).toBeVisible();
    await expect(page.getByTestId('plan-free')).toBeVisible();
    await expect(page.getByTestId('plan-pro')).toBeVisible();
    await expect(page.getByTestId('plan-pro')).toContainText('¥99');

    // 2. 钱包余额呈现与充值包选择
    const balance = page.getByTestId('wallet-balance');
    await expect(balance).toBeVisible({ timeout: 15_000 });
    await expect(balance).toContainText('¥25.00');

    await page.getByTestId('pack-100').click();
    await expect(page.getByTestId('pack-100')).toHaveClass(/is-active/);

    await page.getByTestId('provider-creem').click();
    await expect(page.getByTestId('provider-creem')).toHaveClass(/is-active/);

    // 3. 点击充值生成结账链接
    await page.getByTestId('btn-start-topup').click();
    const redirectBox = page.getByTestId('checkout-redirect-box');
    await expect(redirectBox).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('pay-link')).toBeVisible();

    expect(errors).toEqual([]);
  });
});

test.describe('拦截墙与支付成功回跳（E3.4）', () => {
  test('拦截墙展示升级说明与通道；成功页正确呈现订单号并引导返回对话', async ({ page }) => {
    const errors = collectPageErrors(page);

    // 1. 访问拦截墙
    await gotoBilling(page, '/upgrade/paywall');
    await expect(page.getByTestId('paywall-page')).toBeVisible();
    await expect(page.getByTestId('goto-pricing-btn')).toBeVisible();
    await expect(page.getByTestId('goto-topup-btn')).toBeVisible();

    // 2. 访问支付成功回调页
    await gotoBilling(page, '/upgrade/success?order_id=ord_test_888', 'poc-test-token');
    await expect(page.getByTestId('upgrade-success-page')).toBeVisible();
    const orderId = page.getByTestId('success-order-id');
    await expect(orderId).toBeVisible();
    await expect(orderId).toContainText('ord_test_888');
    await expect(page.getByTestId('back-to-chat-btn')).toBeVisible();

    expect(errors).toEqual([]);
  });
});

test.describe('桌面端购买引导（E3.4）', () => {
  test('桌面端购买页说明免费客户端与云端升级入口', async ({ page }) => {
    const errors = collectPageErrors(page);
    await gotoBilling(page, '/desktop/buy');
    await expect(page.getByTestId('desktop-buy-page')).toBeVisible();
    await expect(page.getByTestId('goto-cloud-pricing')).toBeVisible();

    expect(errors).toEqual([]);
  });
});
