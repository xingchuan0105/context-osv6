import { test, type Browser, type Page } from "@playwright/test";

import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { clearLlmSecrets, readLocalSessionToken } from "./helpers";
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

test("desktop chat reports no llm client when no provider secret is set", async () => {
  test.setTimeout(180_000);
  await clearLlmSecrets(readLocalSessionToken());

  const workbench = new DesktopWorkbench(page);
  await workbench.goToDashboard();
  await workbench.createWorkspace();
  await workbench.sendMessage("ping");
  // PR-4: with no platform key and no llm secret, the local API's execute path
  // reports "LLM client is not configured" (was "LLM is not configured" in legacy).
  await workbench.waitForChatError(/LLM client is not configured/i);
});
