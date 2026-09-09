import { test, expect } from '@playwright/test';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { seedNextAuth, nextAuthPayload } from './auth-seed';

const fixture = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;
const source = readFileSync('../../crates/web-ui/src/app.rs', 'utf8');
const routes = [...source.matchAll(/path!\("([^"]+)"\)/g)].map((match) => match[1]
  .replace('/:session_id?', '').replace(':workspace_id', 'ws-materials')
  .replace(':member_id', 'mem-99').replace(':owner_user_id', 'fixture-user')
  .replace(':user_id', 'u-materials-lead').replace(':token', 'tok-valid-123'));
routes.push('/settings?tab=providers', '/settings?tab=preferences', '/settings?tab=security', '/settings?tab=billing');
const phase = process.env.VISUAL_AUDIT_PHASE || 'after';
const output = `/tmp/context-rust-visual/${phase}`;

for (const theme of ['light', 'dark']) {
  for (const width of [390, 1280]) {
    test(`controls and overlays: ${theme} ${width}px`, async ({ page, request }) => {
      mkdirSync(output, { recursive: true });
      await request.post(`${fixture}/admin/reset`);
      await seedNextAuth(page, 'poc-test-token');
      await page.setViewportSize({ width, height: 844 });
      await page.addInitScript(({ fixture, theme }) => {
        (window as any).__POC_CHAT_API_BASE__ = fixture;
        localStorage.setItem('avrag.ui.theme.v1', theme);
      }, { fixture, theme });
      await page.goto('/chat');
      await expect(page.getByTestId('chat-composer')).toHaveCSS('background-color', theme === 'dark' ? 'rgb(26, 26, 26)' : 'rgb(255, 255, 255)');
      await page.evaluate(() => document.fonts.ready);
      expect(await page.evaluate(() => [...document.fonts].some(font => font.family === 'Inter' && font.status === 'loaded'))).toBe(true);
      await expect(page.getByTestId('send-button')).toBeDisabled();
      await page.getByTestId('composer-input').fill('A draft for visual inspection');
      await expect(page.getByTestId('send-button')).toBeEnabled();
      await page.getByTestId('scope-cap-search').click();
      await expect(page.getByTestId('scope-cap-search')).toHaveAttribute('aria-pressed', 'true');
      await page.mouse.move(0, 0);
      const selectedBackground = theme === 'dark' ? 'rgb(31, 31, 31)' : 'rgb(247, 231, 222)';
      await expect(page.getByTestId('scope-cap-search')).toHaveCSS('background-color', selectedBackground);
      await page.getByTestId('scope-cap-search').hover();
      await expect(page.getByTestId('scope-cap-search')).toHaveCSS('background-color', selectedBackground);
      await page.screenshot({ path: `${output}/${theme}-${width}-composer-active.png`, fullPage: true, animations: 'disabled' });
      const account = page.getByTestId('dashboard-account-menu-trigger');
      if (width === 390 && !(await account.isVisible())) await page.getByTestId('chat-rail-toggle').click();
      await account.click();
      const menu = page.getByTestId('dashboard-account-menu');
      await expect(menu).toBeVisible();
      const menuBox = await menu.boundingBox();
      expect(menuBox!.x).toBeGreaterThanOrEqual(0);
      expect(menuBox!.x + menuBox!.width).toBeLessThanOrEqual(width);
      await page.screenshot({ path: `${output}/${theme}-${width}-account-menu.png`, fullPage: true, animations: 'disabled' });
      await page.keyboard.press('Escape');
      await expect(menu).toBeHidden();
      await expect(account).toBeFocused();
      await account.click();
      await page.mouse.click(width - 2, 842);
      await expect(menu).toBeHidden();
      await expect(account).toBeFocused();
      if (width === 390) {
        await page.keyboard.press('Escape');
        await page.getByTestId('chat-rail-toggle').click();
        const dismiss = page.getByTestId('chat-rail-dismiss');
        await expect(dismiss).toBeVisible();
        expect((await dismiss.boundingBox())!.height).toBe(844);
        await page.mouse.click(width - 2, 400);
        await expect(dismiss).toBeHidden();
      }
      await page.goto('/dashboard');
      await page.getByTestId('create-workspace-btn').click();
      const dialog = page.getByTestId('create-workspace-dialog');
      await expect(dialog).toBeVisible();
      await expect(dialog).toHaveCSS('border-radius', '8px');
      expect(await dialog.evaluate(el => getComputedStyle(el, '::backdrop').backgroundColor)).toBe('rgba(0, 0, 0, 0.4)');
      expect((await dialog.boundingBox())!.width).toBeLessThanOrEqual(width - 16);
      await page.screenshot({ path: `${output}/${theme}-${width}-create-dialog.png`, fullPage: true, animations: 'disabled' });
      await page.keyboard.press('Escape');
      await expect(dialog).toBeHidden();
      await expect(page.getByTestId('create-workspace-btn')).toBeFocused();
    });

    test(`all routes: ${theme} ${width}px`, async ({ page, request }) => {
      test.setTimeout(300_000);
      mkdirSync(output, { recursive: true });
      await request.post(`${fixture}/admin/reset`);
      await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
      await page.addInitScript(({ fixture, theme, auth }) => {
        (window as any).__POC_CHAT_API_BASE__ = fixture;
        localStorage.setItem('avrag.ui.theme.v1', theme);
        const anonymous = /^\/(login|register|reset-password)(\/|$)/.test(location.pathname);
        if (anonymous) {
          localStorage.removeItem('avrag.auth.v1');
          document.cookie = 'avrag.auth.session=; Path=/; Max-Age=0';
          document.cookie = 'avrag.auth.persisted=; Path=/; Max-Age=0';
        } else {
          localStorage.setItem('avrag.auth.v1', JSON.stringify(auth));
          document.cookie = 'avrag.auth.session=1; Path=/; SameSite=Lax';
          document.cookie = `avrag.auth.persisted=${encodeURIComponent(JSON.stringify(auth))}; Path=/; SameSite=Lax`;
        }
      }, { fixture, theme, auth: nextAuthPayload('poc-test-token') });
      const report: unknown[] = [];
      for (const path of [...new Set(routes)]) {
        await page.goto(path, { waitUntil: 'networkidle' });
        const state = await page.evaluate(() => {
          const visible = (el: Element) => el.getBoundingClientRect().width > 0 && el.getBoundingClientRect().height > 0;
          const describe = (el: Element) => `${el.tagName}.${el.className}: ${el.textContent?.trim().slice(0, 45)}`;
          return {
            overflow: document.documentElement.scrollWidth - innerWidth,
            heavy: [...document.querySelectorAll('h1,h2,h3,h4,strong,b,th,button')].filter(visible).filter(el => Number(getComputedStyle(el).fontWeight) > 400).map(describe),
            headers: document.querySelectorAll('[data-testid="app-top-bar"],.public-layout-header,.mkt-chrome').length,
            applicationFooters: document.querySelectorAll('.application-layout [data-testid="product-chrome-footer"]').length,
            wide: [...document.querySelectorAll('main *,section *,form *')].filter(visible).filter(el => el.getBoundingClientRect().right > innerWidth + 1 && !el.closest('pre,.admin-table-scroll,.pub-table-wrap')).slice(0, 8).map(describe),
          };
        });
        report.push({ path, finalUrl: page.url(), ...state });
        await page.screenshot({ path: `${output}/${theme}-${width}-${path.replaceAll('/', '_').replaceAll('?', '_')}.png`, fullPage: true, animations: 'disabled' });
        if (phase !== 'before') {
          expect.soft(state.overflow, `${path}: horizontal overflow`).toBeLessThanOrEqual(1);
          expect.soft(state.heavy, `${path}: browser-default bold`).toEqual([]);
          expect.soft(state.headers, `${path}: one page header`).toBe(1);
          expect.soft(state.applicationFooters, `${path}: no marketing footer in app`).toBe(0);
        }
      }
      writeFileSync(`${output}/${theme}-${width}.json`, JSON.stringify(report, null, 2));
    });
  }
}

test('attachment control stays compact and long filenames do not widen the composer', async ({ page, request }) => {
  await request.post(`${fixture}/admin/reset`);
  await seedNextAuth(page, 'poc-test-token');
  await page.addInitScript(base => { (window as any).__POC_CHAT_API_BASE__ = base; }, fixture);
  await page.route('**/api/v1/chat/attachments/parse?*', route => route.fulfill({ json: { filename: '季度经营分析'.repeat(25) + '.xlsx', mime_type: 'text/plain', text: 'Revenue: 100' } }));
  for (const width of [390, 1280]) {
    await page.setViewportSize({ width, height: 844 });
    await page.goto('/chat');
    const add = page.getByTestId('turn-attachment-add');
    await expect(add).toBeEnabled();
    const box = await add.boundingBox();
    expect(box!.width).toBeLessThanOrEqual(120);
    expect(box!.height).toBeGreaterThanOrEqual(32);
    await page.getByTestId('turn-attachment-input').setInputFiles({ name: 'test.xlsx', mimeType: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', buffer: Buffer.from('fixture') });
    await expect(page.getByTestId('turn-attachment-item')).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth - innerWidth)).toBeLessThanOrEqual(1);
    await page.getByTestId('turn-attachment-item').getByRole('button').click();
    await expect(page.getByTestId('turn-attachment-item')).toHaveCount(0);
  }
});
