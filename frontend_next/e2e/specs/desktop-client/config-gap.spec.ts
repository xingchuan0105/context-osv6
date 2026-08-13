import { test, expect, type Browser, type Page } from "@playwright/test";

import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { listProviderSecrets, readLocalSessionToken, upsertDummyProviderSecret } from "./helpers";
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

test("provider secret is visible and desktop chat runs via the local API", async () => {
  test.setTimeout(240_000);

  const token = readLocalSessionToken();
  const savedSecret = await upsertDummyProviderSecret(token);
  expect(savedSecret.key_fingerprint).toMatch(/:[0-9]+$/);
  const listed = await listProviderSecrets(token);
  expect(
    listed.secrets.some(
      (secret) =>
        secret.purpose === "llm" &&
        secret.provider === "custom" &&
        secret.key_fingerprint === savedSecret.key_fingerprint,
    ),
  ).toBe(true);

  const workbench = new DesktopWorkbench(page);
  await workbench.goToDashboard();
  await workbench.createWorkspace();
  await workbench.sendMessage("ping");

  // PR-4 aligned: chat now reaches the local avrag-api and constructs the client
  // from the resolved secret (G1). The dead 127.0.0.1:9 endpoint fails the model
  // call — but never the legacy "LLM is not configured" path, and never a hang.
  const errorText = await workbench.waitForAnyChatError(20_000);
  expect(errorText).not.toMatch(/not configured/i);
});
