import { test, expect, type Browser, type Page } from "@playwright/test";

import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { readLocalSessionToken, upsertProviderSecret } from "./helpers";
import { connectTauriPage } from "./webview";

const LLM_API_KEY = process.env.DESKTOP_E2E_LLM_API_KEY;
const LLM_BASE_URL = process.env.DESKTOP_E2E_LLM_BASE_URL ?? "https://api.deepseek.com";
const LLM_MODEL = process.env.DESKTOP_E2E_LLM_MODEL ?? "deepseek-v4-flash";
const EMBED_API_KEY = process.env.DESKTOP_E2E_EMBED_API_KEY;
const EMBED_BASE_URL = process.env.DESKTOP_E2E_EMBED_BASE_URL ?? "https://api.siliconflow.cn/v1";
const EMBED_MODEL = process.env.DESKTOP_E2E_EMBED_MODEL ?? "BAAI/bge-m3";
const FIXTURE = process.env.DESKTOP_E2E_FIXTURE ?? "";

const run = Boolean(LLM_API_KEY && EMBED_API_KEY && FIXTURE);

let browser: Browser;
let page: Page;

test.beforeAll(async () => {
  test.skip(
    !run,
    "D-rag-full requires DESKTOP_E2E_LLM_API_KEY + DESKTOP_E2E_EMBED_API_KEY + DESKTOP_E2E_FIXTURE",
  );
  const attached = await connectTauriPage();
  browser = attached.browser;
  page = attached.page;
});

test.afterAll(async () => {
  await browser?.close();
});

async function restartLocalProduct(page: Page) {
  await page.evaluate(async () => {
    const w = window as unknown as {
      __TAURI__?: { core?: { invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } };
    };
    const invoke = w.__TAURI__?.core?.invoke;
    if (!invoke) {
      throw new Error("Tauri invoke not exposed in webview");
    }
    await invoke("restart_local_product");
  });
}

test("D-rag-full: grounded answer with citations from real llm + embedding secrets", async () => {
  test.setTimeout(600_000);
  const token = readLocalSessionToken();

  // 1) real secrets (not the PR-3 dummy rows)
  await upsertProviderSecret(token, {
    purpose: "llm",
    provider: "deepseek",
    api_key: LLM_API_KEY as string,
    base_url: LLM_BASE_URL,
    model_hint: LLM_MODEL,
    workspace_id: null,
  });
  await upsertProviderSecret(token, {
    purpose: "embedding",
    provider: "siliconflow",
    api_key: EMBED_API_KEY as string,
    base_url: EMBED_BASE_URL,
    model_hint: EMBED_MODEL,
    workspace_id: null,
  });

  // 2) restart the local product so bootstrap resolves the embedding secret (G6).
  //    `restart_local_product` resolves only after stop + ensure complete.
  await restartLocalProduct(page);

  const workbench = new DesktopWorkbench(page);
  await workbench.goToDashboard();
  await workbench.createWorkspace();

  // 3) upload the standard corpus (RAG is on after the restart)
  await workbench.uploadFile(FIXTURE);
  await workbench.waitForIngestionComplete();

  // 3b) RAG requires explicit source selection + the rag capability chip
  await workbench.selectFirstSource();
  await workbench.enableRagMode();

  // 4) grounded question on the antifragile corpus → non-empty assistant + citation
  await workbench.sendMessage("What does the uploaded document say about antifragility?");
  await workbench.waitForAssistantMessage();
  await expect(page.locator('[data-testid="workspace-citation"]').first()).toBeVisible();
});
