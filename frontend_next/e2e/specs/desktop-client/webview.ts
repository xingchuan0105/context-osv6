import { chromium, type Browser, type Page } from "@playwright/test";

const CDP_URL = process.env.DESKTOP_E2E_CDP_URL ?? "http://127.0.0.1:19322";
const ATTACH_TIMEOUT_MS = Number(process.env.DESKTOP_E2E_CDP_ATTACH_TIMEOUT_MS ?? 120_000);
const TAURI_ORIGIN = /^https?:\/\/tauri\.localhost\//i;

export async function connectTauriPage(): Promise<{ browser: Browser; page: Page }> {
  const deadline = Date.now() + ATTACH_TIMEOUT_MS;
  while (Date.now() < deadline) {
    try {
      const browser = await chromium.connectOverCDP(CDP_URL);
      const page = findTauriPage(browser);
      if (page) {
        await page.waitForLoadState("domcontentloaded").catch(() => {});
        return { browser, page };
      }
      await browser.close();
    } catch {
      // CDP becomes available after the WebView2 process starts.
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error(`no tauri.localhost target found at ${CDP_URL}`);
}

function findTauriPage(browser: Browser): Page | null {
  for (const context of browser.contexts()) {
    for (const page of context.pages()) {
      try {
        if (TAURI_ORIGIN.test(page.url())) {
          return page;
        }
      } catch {
        // CDP targets can disappear while app bootstrap is still running.
      }
    }
  }
  return null;
}
