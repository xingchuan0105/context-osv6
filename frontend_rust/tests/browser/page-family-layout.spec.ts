import {test, expect} from '@playwright/test';
import {seedNextAuth} from './auth-seed';
const fixture = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

for (const width of [390, 1280]) {
  for (const theme of ['light', 'dark']) {
    test(`W4 page families ${width} ${theme}`, async ({page, request}) => {
      await request.post(`${fixture}/admin/reset`);
      await seedNextAuth(page, 'poc-test-token');
      await page.setViewportSize({width, height:800});
      await page.addInitScript(({fixture,theme}) => {
        (window as any).__POC_CHAT_API_BASE__=fixture;
        localStorage.setItem('avrag.ui.theme.v1',theme);
      }, {fixture,theme});
      const errors:string[]=[];
      page.on('pageerror', error => errors.push(error.message));
      for (const path of ['/admin/accounts','/pricing','/help','/help/faq','/integrations/mcp','/desktop','/activate','/setup','/legal/terms','/en/legal/terms','/upgrade/paywall','/upgrade/success','/desktop/buy']) {
        await page.goto(path);
        if (path === '/activate') await expect(page).toHaveURL(/\/desktop$/);
        if (path === '/setup') await expect(page).toHaveURL(/\/settings\?tab=providers$/);
        if (path.startsWith('/admin') || path === '/help' || path === '/setup') {
          await expect(page.getByTestId('app-top-bar')).toHaveCount(1);
          await expect(page.getByTestId('product-chrome-footer')).toHaveCount(0);
        } else {
          await expect(page.locator('.public-layout-header, .mkt-chrome')).toHaveCount(1);
          await expect(page.getByTestId('session-list')).toHaveCount(0);
        }
        if (path.startsWith('/admin')) {
          await expect(page.locator('.admin-nav [aria-current="page"]')).toHaveAttribute('href','/admin/accounts');
          await page.getByTestId('admin-loading').waitFor({state:'hidden'});
        }
        if (path.includes('/legal/terms') && width < 768) {
          const content = await page.locator('.legal-content').boundingBox();
          const toc = await page.locator('.legal-toc').boundingBox();
          expect(content!.width).toBeGreaterThan(width * 0.8);
          expect(content!.y).toBeGreaterThanOrEqual(toc!.y + toc!.height);
        }
        expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth), path).toBeLessThanOrEqual(1);
        await page.screenshot({path:`/tmp/context-rust-w4-visual/${path.replaceAll('/','_')}-${width}-${theme}.png`, animations:'disabled'});
      }
      expect(errors).toEqual([]);
    });
    test(`anonymous authentication layout ${width} ${theme}`, async ({page}) => {
      await page.setViewportSize({width, height:800});
      await page.addInitScript(({fixture,theme}) => {
        (window as any).__POC_CHAT_API_BASE__=fixture;
        localStorage.setItem('avrag.ui.theme.v1',theme);
      }, {fixture,theme});
      for (const path of ['/login','/register','/reset-password']) {
        await page.goto(path);
        await expect(page.locator('.public-layout-header')).toBeVisible();
        await expect(page.locator('.auth-card')).toBeVisible();
        await expect(page.getByTestId('session-list')).toHaveCount(0);
        expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
        await page.screenshot({path:`/tmp/context-rust-w4-visual/${path.replaceAll('/','_')}-${width}-${theme}.png`, animations:'disabled'});
      }
    });
  }
}
