import { test, expect, type Browser, type Page } from "@playwright/test";
import fs from "fs";
import os from "os";
import path from "path";

import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { connectTauriPage } from "./webview";

const CLOUD_EMAIL = process.env.DESKTOP_E2E_CLOUD_EMAIL;
const CLOUD_PASSWORD = process.env.DESKTOP_E2E_CLOUD_PASSWORD;
const PROBE_MARKER = "Qilian purple lantern rings at winter solstice 20260819b3b";
const CHROME_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const run = Boolean(CLOUD_EMAIL && CLOUD_PASSWORD);

let browser: Browser;
let page: Page;

test.beforeAll(async () => {
  test.skip(!run, "publish-share requires DESKTOP_E2E_CLOUD_EMAIL + DESKTOP_E2E_CLOUD_PASSWORD");
  const attached = await connectTauriPage();
  browser = attached.browser;
  page = attached.page;
});

test.afterAll(async () => {
  await browser?.close();
});

test("desktop publish-share: gate → login → ingest → publish → cloud share RAG", async () => {
  test.setTimeout(600_000);

  const emailField = page.getByLabel(/邮箱|账号邮箱/);
  await emailField.waitFor({ state: "visible", timeout: 120_000 });
  await emailField.fill(CLOUD_EMAIL as string);
  await page.getByLabel("密码").fill(CLOUD_PASSWORD as string);
  await page.getByRole("button", { name: /登录/ }).click();
  await emailField.waitFor({ state: "hidden", timeout: 120_000 });

  const workbench = new DesktopWorkbench(page);
  await page.goto("http://tauri.localhost/dashboard");
  await page.getByTestId("dashboard-create-workspace").waitFor({ state: "visible", timeout: 300_000 });
  await workbench.createWorkspace();

  const fixtureDir = process.env.DESKTOP_E2E_APP_DATA_BACKUP || os.tmpdir();
  const fixturePath = path.join(fixtureDir, "b3b-probe.txt");
  fs.writeFileSync(fixturePath, `${PROBE_MARKER}\n`, "utf8");
  await workbench.uploadFile(fixturePath);
  await workbench.waitForIngestionComplete(300_000);

  await page.getByTestId("workspace-topbar-share").click();
  const publishCta = page.getByTestId("desktop-publish-cta");
  await publishCta.waitFor({ state: "visible", timeout: 30_000 });
  await publishCta.click();
  await page.getByTestId("desktop-publish-gate").waitFor({ state: "hidden", timeout: 300_000 });

  const shareSwitch = page.getByTestId("share-switch");
  await expect(shareSwitch).toBeEnabled({ timeout: 30_000 });
  if ((await shareSwitch.getAttribute("aria-checked")) !== "true") {
    await shareSwitch.click();
    await page.getByTestId("share-enable-confirm-action").click();
  }

  const visitorMode = page.getByTestId("share-visitor-mode");
  await visitorMode.selectOption("anonymous");

  const shareLink = page.getByTestId("share-link");
  await expect(shareLink).toHaveText(/https:\/\/app\.contextlm\.top\/shared\/kb\//, { timeout: 30_000 });
  const shareUrl = ((await shareLink.textContent()) ?? "").trim();
  const token = shareUrl.split("/shared/kb/")[1]?.split(/[?#]/)[0];
  expect(token, `share url should contain a token: ${shareUrl}`).toBeTruthy();

  const shared = await cloudJson(`/api/shared/kb/${token}`);
  const payload = (shared.data ?? shared) as {
    knowledge_base?: { id?: string };
    sources?: Array<{ id?: string }>;
  };
  const workspaceId = payload.knowledge_base?.id ?? "";
  const docIds = (payload.sources ?? []).map((source) => source.id).filter((id): id is string => Boolean(id));
  expect(workspaceId).toBeTruthy();
  expect(docIds.length).toBeGreaterThan(0);

  const answer = await cloudShareRag(token as string, workspaceId, docIds, `What date does the ${PROBE_MARKER} mention?`);
  expect(answer.toLowerCase()).toMatch(/winter solstice|冬至/);
});

async function cloudJson(pathName: string): Promise<Record<string, unknown>> {
  const response = await fetch(`https://app.contextlm.top${pathName}`, {
    headers: { "User-Agent": CHROME_UA, Accept: "application/json" },
  });
  expect(response.ok, `GET ${pathName} → ${response.status}`).toBeTruthy();
  return (await response.json()) as Record<string, unknown>;
}

async function cloudShareRag(
  shareToken: string,
  workspaceId: string,
  docIds: string[],
  query: string,
): Promise<string> {
  const response = await fetch("https://app.contextlm.top/api/v1/chat", {
    method: "POST",
    headers: {
      "User-Agent": CHROME_UA,
      Accept: "text/event-stream",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      query,
      workspace_id: workspaceId,
      session_id: null,
      agent_type: "rag",
      source_type: "share",
      source_token: shareToken,
      doc_scope: docIds,
      messages: [],
      stream: true,
    }),
  });
  expect(response.ok, `share chat → ${response.status}`).toBeTruthy();
  const text = await response.text();
  const chunks: string[] = [];
  for (const line of text.split("\n")) {
    if (!line.startsWith("data:")) {
      continue;
    }
    const raw = line.slice(5).trim();
    if (!raw || raw === "[DONE]") {
      continue;
    }
    try {
      const event = JSON.parse(raw) as { type?: string; delta?: string; content?: string; text?: string };
      const piece = event.delta ?? event.content ?? event.text ?? "";
      if (piece) {
        chunks.push(piece);
      }
    } catch {
      chunks.push(raw);
    }
  }
  return chunks.join("");
}
