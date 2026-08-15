import { test, expect, type Browser, type Page } from "@playwright/test";
import fs from "fs";
import path from "path";
import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { connectTauriPage } from "./webview";
import {
  expectNoTauriExternalBrowser,
  snapshotExternalBrowserCandidates,
} from "./external-browser";

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

test("desktop shell creates workspace in WebView and uploads via IPC", async () => {
  test.setTimeout(240_000);

  const failedUploadRequests: string[] = [];
  const failedFetchText: string[] = [];
  page.on("requestfailed", (request) => {
    if (/127\.0\.0\.1:18080\/uploads\//.test(request.url())) {
      failedUploadRequests.push(`${request.url()} ${request.failure()?.errorText ?? ""}`);
    }
  });
  page.on("console", (message) => {
    if (message.type() === "error" && /Failed to fetch/i.test(message.text())) {
      failedFetchText.push(message.text());
    }
  });

  const fixturePath =
    process.env.DESKTOP_E2E_FIXTURE || path.join(__dirname, "../../fixtures/antifragile.txt");
  const workbench = new DesktopWorkbench(page);

  await workbench.goToDashboard();
  expect(page.url()).not.toMatch(/login|activate/i);
  const browserBefore = snapshotExternalBrowserCandidates();
  await workbench.createWorkspace();
  expectNoTauriExternalBrowser(
    browserBefore,
    snapshotExternalBrowserCandidates(),
  );
  await workbench.expectWorkspaceUrlHasWorkspaceId();
  await workbench.uploadFile(fixturePath);

  expect(failedUploadRequests, "WebView must not fetch /uploads directly").toEqual([]);
  expect(failedFetchText, "UI must not show Failed to fetch").toEqual([]);

  await workbench.waitForIngestionComplete();

  expect(failedUploadRequests).toEqual([]);
  expect(failedFetchText).toEqual([]);
  expect(page.url()).toMatch(/\/dashboard\/_placeholder\?ws=/);

  const documentId = await page
    .locator(
      '[data-testid="ingestion-status"][data-status="completed"], [data-testid="ingestion-status"][data-status="ready"]',
    )
    .first()
    .getAttribute("data-document-id");
  if (!documentId) {
    throw new Error("desktop ingestion row missing data-document-id");
  }
  const token = readLocalSessionToken();
  await expectDesktopDocumentCompleted(documentId, token);
});

function readLocalSessionToken(): string {
  const appData =
    process.env.APPDATA ?? path.join(process.env.USERPROFILE ?? "", "AppData", "Roaming");
  const sessionPath = path.join(appData, "com.contextos.desktop", "local_session.json");
  const session = JSON.parse(fs.readFileSync(sessionPath, "utf8")) as { token?: string };
  if (!session.token) {
    throw new Error("desktop local_session.json has no JWT");
  }
  return session.token;
}

async function expectDesktopDocumentCompleted(documentId: string, token: string) {
  const deadline = Date.now() + 180_000;
  while (Date.now() < deadline) {
    const response = await fetch(
      `http://127.0.0.1:18080/api/v1/documents/${documentId}/status`,
      {
        headers: { Authorization: `Bearer ${token}` },
      },
    );
    if (response.ok) {
      const body = (await response.json()) as { status?: string };
      if (["completed", "ready", "Completed", "Ready"].includes(body.status ?? "")) {
        return;
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 2_000));
  }
  throw new Error(`document ${documentId} did not reach completed status`);
}
