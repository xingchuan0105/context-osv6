import { test, expect, type Browser, type Page } from "@playwright/test";

import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { connectTauriPage } from "./webview";

/**
 * l3 cloud-login acceptance (2026-08-15 wave, W3 真机门 / W5): the real W3
 * login gate owns the shell (run.sh l3 passes -NoCloudGateBypass, and the
 * legacy BYOK seed is skipped), so the RAG answer below can only come from
 * the official relay — official keys metered against the cloud wallet.
 *
 * Cloud creds come from avrag-rs/.env (DESKTOP_E2E_CLOUD_*) via run.sh.
 * The harness backup/restore hides any pre-existing cloud_session.json, so
 * the gate always shows the login card here.
 */

const CLOUD_EMAIL = process.env.DESKTOP_E2E_CLOUD_EMAIL;
const CLOUD_PASSWORD = process.env.DESKTOP_E2E_CLOUD_PASSWORD;
const FIXTURE = process.env.DESKTOP_E2E_FIXTURE ?? "";

const run = Boolean(CLOUD_EMAIL && CLOUD_PASSWORD && FIXTURE);

let browser: Browser;
let page: Page;

test.beforeAll(async () => {
  test.skip(
    !run,
    "cloud-login requires DESKTOP_E2E_CLOUD_EMAIL + DESKTOP_E2E_CLOUD_PASSWORD + DESKTOP_E2E_FIXTURE",
  );
  const attached = await connectTauriPage();
  browser = attached.browser;
  page = attached.page;
});

test.afterAll(async () => {
  await browser?.close();
});

test("cloud-login: gate → login → official-relay RAG answer with citations", async () => {
  test.setTimeout(600_000);

  // 1) Without a cloud session the shell renders only the W3 login card.
  const emailField = page.getByLabel("云账户邮箱");
  await emailField.waitFor({ state: "visible", timeout: 120_000 });

  // 2) Login runs Rust-side (reqwest): session JWT → desktop token →
  //    relay-config → client.env relay block. Wait for the gate to release —
  //    navigating away first would abandon the in-flight gate state, and the
  //    re-mounted card never rechecks the session the IPC may have saved.
  await emailField.fill(CLOUD_EMAIL as string);
  await page.getByLabel("密码").fill(CLOUD_PASSWORD as string);
  await page.getByRole("button", { name: "登录并启用官方模型" }).click();
  await emailField.waitFor({ state: "hidden", timeout: 120_000 });

  // 3) Gate released → local session bootstrap → workspace UI. The bootstrap
  //    now cold-starts the whole stack (initdb + migrations + product), so
  //    the dashboard can take minutes to appear.
  const workbench = new DesktopWorkbench(page);
  await page.goto("http://tauri.localhost/dashboard");
  await page
    .getByTestId("dashboard-create-workspace")
    .waitFor({ state: "visible", timeout: 300_000 });
  await workbench.createWorkspace();

  // 4) RAG with no BYOK configured — the answer is served by official relay
  //    keys (wallet-metered); a missing relay would surface as a chat error.
  await workbench.uploadFile(FIXTURE);
  await workbench.waitForIngestionComplete();
  await workbench.selectFirstSource();
  await workbench.enableRagMode();

  await workbench.sendMessage("What does the uploaded document say about antifragility?");
  await workbench.waitForAssistantMessage(300_000);
  await expect(page.locator('[data-testid="workspace-citation"]').first()).toBeVisible();
});
