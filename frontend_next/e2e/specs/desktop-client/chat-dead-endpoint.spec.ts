import { test, expect, type Browser, type Page } from "@playwright/test";

import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { readLocalSessionToken, upsertDummyProviderSecret } from "./helpers";
import { connectTauriPage } from "./webview";

let browser: Browser;
let page: Page;

test.beforeAll(async () => {
  const attached = await connectTauriPage();
  browser = attached.browser;
  page = attached.page;
});

test.afterAll(async () => {
  await browser?.close();
});

test("desktop chat fails fast when the provider secret points at a dead endpoint", async () => {
  test.setTimeout(180_000);
  await upsertDummyProviderSecret(readLocalSessionToken());

  const workbench = new DesktopWorkbench(page);
  await workbench.goToDashboard();
  await workbench.createWorkspace();
  await workbench.sendMessage("ping");

  // Dead endpoint (127.0.0.1:9) must fail the model call quickly — not hang and
  // not fall back to "not configured" (which would mean the secret was ignored).
  const errorText = await workbench.waitForAnyChatError(20_000);
  expect(errorText).not.toMatch(/not configured/i);
});
