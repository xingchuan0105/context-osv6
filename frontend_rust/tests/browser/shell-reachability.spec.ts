import { test, expect, type Page } from '@playwright/test';
import { seedNextAuth } from './auth-seed';

const WEB_BASE = `http://127.0.0.1:${Number(process.env.POC_WEB_PORT || 3200)}`;
const FIXTURE_BASE = `http://127.0.0.1:${Number(process.env.POC_FIXTURE_PORT || 3201)}`;

const CANONICAL = [
  '/chat',
  '/dashboard',
  '/dashboard/analytics',
  '/settings',
  '/pricing',
  '/pricing#topup',
  '/desktop',
  '/help',
  '/help/api-access',
  '/legal',
  '/legal/terms',
  '/legal/privacy',
  '/legal/licenses',
];

async function collectHrefs(page: Page): Promise<Set<string>> {
  const hrefs = await page.$$eval('a[href]', (nodes) =>
    nodes
      .map((node) => node.getAttribute('href') || '')
      .filter((href) => href.startsWith('/')),
  );
  return new Set(hrefs.map((href) => href.split('?')[0]));
}

async function collectChromeHrefs(page: Page): Promise<Set<string>> {
  const hrefs = await collectHrefs(page);
  const share = page.getByTestId('app-topbar-share-menu');
  if (await share.count()) {
    await share.click();
    await expect(page.getByTestId('app-topbar-share-menu-panel')).toBeVisible();
    for (const href of await collectHrefs(page)) hrefs.add(href);
    await page.keyboard.press('Escape');
    const dismiss = page.locator('.app-menu-dismiss').first();
    if (await dismiss.isVisible()) await dismiss.click();
  }
  const account = page.getByTestId('dashboard-account-menu-trigger');
  if (await account.count()) {
    await account.click();
    await expect(page.getByTestId('dashboard-account-menu')).toBeVisible();
    for (const href of await collectHrefs(page)) hrefs.add(href);
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('dashboard-account-menu')).toBeHidden();
  }
  return hrefs;
}

test.describe('E5.2 全局壳可达性', () => {
  test('/chat 顶栏与 Workspaces 入口齐备', async ({ page }) => {
    await seedNextAuth(page, 'fixture-token');
    await page.goto(`${WEB_BASE}/chat`, { waitUntil: 'domcontentloaded' });
    await expect(page.getByTestId('app-top-bar')).toBeVisible();
    await expect(page.getByTestId('all-workspaces-link')).toHaveAttribute('href', '/dashboard');
    await expect(page.getByTestId('app-topbar-share-menu')).toHaveCount(0);
    await page.getByTestId('dashboard-account-menu-trigger').click();
    await expect(page.getByTestId('account-settings-link')).toHaveAttribute('href', '/settings');
    await expect(page.getByTestId('account-help-link')).toHaveAttribute('href', '/help');
  });

  test('产品页与营销页都能在两跳内到达 canonical 目的地', async ({ page }) => {
    await seedNextAuth(page, 'fixture-token');
    const starts = [
      '/chat',
      '/dashboard',
      '/dashboard/ws-materials',
      '/settings',
      '/pricing',
      '/help',
      '/settings/usage',
    ];
    for (const start of starts) {
      await page.goto(`${WEB_BASE}${start}`, { waitUntil: 'domcontentloaded' });
      if (start === '/pricing') {
        await expect(page.getByTestId('marketing-chrome')).toBeVisible();
      } else {
        await expect(page.getByTestId('app-top-bar')).toBeVisible();
      }
      const first = await collectChromeHrefs(page);
      const missing = CANONICAL.filter((href) => {
        const path = href.split('#')[0];
        return ![...first].some((got) => got === path || got === href);
      });
      if (missing.length === 0) {
        continue;
      }
      const hop = first.has('/dashboard')
        ? '/dashboard'
        : first.has('/help')
          ? '/help'
          : first.has('/chat')
            ? '/chat'
            : null;
      expect(hop, `no second-hop from ${start}, still missing ${missing.join(',')}`).toBeTruthy();
      await page.goto(`${WEB_BASE}${hop}`, { waitUntil: 'domcontentloaded' });
      const second = await collectChromeHrefs(page);
      const union = new Set([...first, ...second]);
      for (const href of missing) {
        const path = href.split('#')[0];
        expect(
          [...union].some((got) => got === path || got === href),
          `${start}: ${href} not reachable in ≤2 hops`,
        ).toBeTruthy();
      }
    }
  });

  test('深层页顶栏存在，返回链不再是唯一出口', async ({ page }) => {
    await seedNextAuth(page, 'fixture-token');
    await page.goto(`${WEB_BASE}/settings/usage`, { waitUntil: 'domcontentloaded' });
    await expect(page.getByTestId('app-top-bar')).toBeVisible();
    await expect(page.getByTestId('usage-page').locator('a.settings-back-link')).toHaveCount(1);
    await page.getByTestId('dashboard-account-menu-trigger').click();
    await expect(page.getByTestId('account-settings-link')).toBeVisible();
  });
});
