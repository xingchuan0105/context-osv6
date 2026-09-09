import { test, expect } from '@playwright/test';
import { seedNextAuth } from './auth-seed';
const fixture = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

test.beforeEach(async ({ page, request }) => {
  await request.post(`${fixture}/admin/reset`);
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, fixture);
});

for (const width of [390, 1280]) {
  for (const theme of ['light', 'dark']) {
    test(`settings categories and deep links ${width} ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 800 });
      await page.addInitScript(theme => localStorage.setItem('avrag.ui.theme.v1', theme), theme);
      await page.goto('/settings');
      const nav = page.locator('.settings-nav');
      await expect(nav).toBeVisible();
      if (width < 768) await expect(page.locator('.settings-content')).toBeHidden();
      await page.getByTestId('tab-preferences').click();
      await expect(page).toHaveURL(/tab=preferences/);
      await expect(page.locator('.settings-content')).toBeVisible();
      if (width < 768) {
        await expect(nav).toBeHidden();
        await page.getByRole('link', {name:'全部设置', exact:true}).click();
        await expect(nav).toBeVisible();
      } else {
        const left = await nav.boundingBox();
        const content = await page.locator('.settings-content').boundingBox();
        expect(content!.x).toBeGreaterThan(left!.x + left!.width);
      }
      await page.goto('/settings?tab=billing');
      await expect(page.getByTestId('billing-goto-pricing')).toHaveAttribute('href', '/pricing');
      await page.goto('/settings?tab=providers');
      await expect(page.locator('.settings-content')).toBeVisible();
      await expect(page.getByTestId('tab-providers')).toHaveAttribute('aria-current', 'page');
      expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
      await page.screenshot({path:`/tmp/context-rust-w3-visual/settings-${width}-${theme}.png`, animations:'disabled'});
      await page.goto('/settings/usage');
      await expect(page.getByTestId('usage-page')).toBeVisible();
      await expect(page.getByTestId('tab-usage')).toHaveAttribute('aria-current', 'page');
      await page.getByTestId('usage-plan').waitFor();
      await expect(page.locator('.settings-usage-card').first()).toHaveCSS('opacity', '1');
      await page.screenshot({path:`/tmp/context-rust-w3-visual/usage-${width}-${theme}.png`, animations:'disabled'});
    });

    test(`share categories retain workspace return path ${width} ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 800 });
      await page.addInitScript(theme => localStorage.setItem('avrag.ui.theme.v1', theme), theme);
      await page.goto('/dashboard/ws-materials/share');
      const nav = page.locator('.settings-nav');
      await expect(nav.getByRole('link')).toHaveCount(3);
      await nav.locator('a[href$="/access-logs"]').click();
      await expect(page.getByTestId('share-logs-page')).toBeVisible();
      await expect(nav.locator('[aria-current="page"]')).toHaveAttribute('href', '/dashboard/ws-materials/share/access-logs');
      await nav.locator('a[href$="/analytics"]').click();
      await expect(page.getByTestId('share-analytics-page')).toBeVisible();
      await page.screenshot({path:`/tmp/context-rust-w3-visual/share-${width}-${theme}.png`, animations:'disabled'});
      expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
      await page.locator('.settings-back-link').click();
      await expect(page).toHaveURL(/\/dashboard\/ws-materials\/share$/);
      await page.locator('.settings-back-link').click();
      await expect(page).toHaveURL(/\/dashboard\/ws-materials$/);
    });
  }
}

test('public pages use a light header without private navigation requests', async ({page}) => {
  const privateReads: string[] = [];
  page.on('request', request => { if (/\/api\/v1\/(sessions|chat\/sessions)(\?|$)/.test(request.url())) privateReads.push(request.url()); });
  for (const width of [390, 1280]) {
  await page.setViewportSize({width, height:800});
  for (const path of ['/shared/kb/tok-valid-123', '/shared/kb/tok-expired', '/shared/u/u-materials-lead', '/invite/ws-materials/mem-99']) {
    await page.goto(path);
    await expect(page.locator('.public-layout-header')).toBeVisible();
    await expect(page.getByTestId('session-list')).toHaveCount(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    await page.screenshot({path:`/tmp/context-rust-w3-visual/public-${path.replaceAll('/', '_')}-${width}.png`, animations:'disabled'});
  }
  }
  expect(privateReads).toEqual([]);
});

test('an unknown settings tab selects the visible profile category', async ({page}) => {
  await page.goto('/settings?tab=unknown');
  await expect(page.getByTestId('tab-profile')).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('.settings-content')).toBeVisible();
  await expect(page.locator('.settings-content')).not.toBeEmpty();
});

test('signed-out settings provide login instead of a logout action', async ({browser}) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto(`http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}/settings?tab=profile`);
  await expect(page.getByTestId('settings-logout')).toBeHidden();
  await expect(page.getByTestId('settings-login-required')).toHaveAttribute('href', '/login?next=/settings');
  await context.close();
});
