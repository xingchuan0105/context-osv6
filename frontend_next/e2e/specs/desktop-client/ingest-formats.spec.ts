import { test, type Browser, type Page } from "@playwright/test";
import fs from "fs";
import path from "path";
import { DesktopWorkbench } from "../../pom/desktop-workbench";
import { connectTauriPage } from "./webview";

// Packaged-parser coverage: the install tree ships runtime/parsers/
// (markitdown-lite / anydoc-lite / lit.exe+pdfium.dll) and the shell writes
// MARKITDOWN_BIN / ANYDOC_BIN / LITEPARSE_BIN into client.env. These specs
// upload one fixture per non-txt route and assert ingestion completes, so a
// missing/broken bundled parser turns red here instead of at a user's desk.

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

test("desktop ingests PDF via the bundled lit parser", async () => {
  test.setTimeout(240_000);
  const documentId = await uploadAndWaitCompleted(
    // Text layer must exceed LITEPARSE_SCANNED_MIN_CHARS (500 non-whitespace),
    // otherwise the worker treats the PDF as scanned and routes to PaddleOCR.
    path.join(__dirname, "../../fixtures/lit-text.pdf"),
  );
  await expectDesktopDocumentCompleted(documentId, readLocalSessionToken());
});

test("desktop ingests DOCX via the bundled anydoc-lite parser", async () => {
  test.setTimeout(240_000);
  const documentId = await uploadAndWaitCompleted(
    path.join(__dirname, "../../fixtures/anydoc-mini.docx"),
  );
  await expectDesktopDocumentCompleted(documentId, readLocalSessionToken());
});

async function uploadAndWaitCompleted(fixturePath: string): Promise<string> {
  const workbench = new DesktopWorkbench(page);
  await workbench.goToDashboard();
  await workbench.createWorkspace();
  await workbench.expectWorkspaceUrlHasWorkspaceId();
  await workbench.uploadFile(fixturePath);
  await workbench.waitForIngestionComplete();
  const documentId = await page
    .locator(
      '[data-testid="ingestion-status"][data-status="completed"], [data-testid="ingestion-status"][data-status="ready"]',
    )
    .first()
    .getAttribute("data-document-id");
  if (!documentId) {
    throw new Error(`ingestion row missing data-document-id for ${fixturePath}`);
  }
  return documentId;
}

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
    const response = await fetch(`http://127.0.0.1:18080/api/v1/documents/${documentId}/status`, {
      headers: { Authorization: `Bearer ${token}` },
    });
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
