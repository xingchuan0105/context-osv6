import { expect, type APIRequestContext, type Page } from "@playwright/test";

export function uniqueName(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

export async function longText(page: Page, label: string, paragraphs = 18): Promise<string> {
  void page;
  return Array.from({ length: paragraphs }, (_, index) =>
    `${label} paragraph ${index + 1}: ` +
    "Knowledge-base answer content that is intentionally long so virtualization has to manage real scrolling pressure."
  ).join("\n\n");
}

export async function registerTestUser(
  request: APIRequestContext,
): Promise<{ token: string; email: string; password: string }> {
  const email = `${uniqueName("pw-user")}@e2e.test`;
  const password = "E2eTest123!";
  const response = await request.post("/api/auth/register", {
    data: { email, password, full_name: "E2E Test User" },
  });
  expect(response.ok(), `register failed: ${await response.text()}`).toBeTruthy();
  const payload = (await response.json()) as { data?: { token?: string } };
  const token = String(payload?.data?.token || "").trim();
  expect(token, "register returned empty token").not.toBe("");
  return { token, email, password };
}

export async function loginTestUser(
  request: APIRequestContext,
  email: string,
  password: string,
): Promise<string> {
  const response = await request.post("/api/auth/login", {
    data: { email, password },
  });
  expect(response.ok(), `login failed: ${await response.text()}`).toBeTruthy();
  const payload = (await response.json()) as { data?: { token?: string } };
  const token = String(payload?.data?.token || "").trim();
  expect(token, "login returned empty token").not.toBe("");
  return token;
}

export async function createNotebookViaAPI(
  request: APIRequestContext,
  token: string,
  name: string,
  description = "e2e test notebook",
): Promise<string> {
  const response = await request.post("/api/v1/notebooks", {
    headers: { Authorization: `Bearer ${token}` },
    data: { name, description },
  });
  expect(response.ok(), `create notebook failed: ${await response.text()}`).toBeTruthy();
  const payload = (await response.json()) as { notebook?: { id?: string } };
  const id = String(payload?.notebook?.id || "");
  expect(id, "notebook id was empty").not.toBe("");
  return id;
}

export async function deleteNotebookViaAPI(
  request: APIRequestContext,
  token: string,
  notebookId: string,
): Promise<void> {
  if (!notebookId) return;
  await request.delete(`/api/v1/notebooks/${notebookId}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
}

export async function uploadDocumentAndWait(
  request: APIRequestContext,
  token: string,
  notebookId: string,
  fileName: string,
  content: Buffer,
  mimeType = "text/plain",
  maxWaitMs = 30_000,
): Promise<string> {
  // Step 1: Create document record
  const createResp = await request.post(
    `/api/v1/notebooks/${notebookId}/documents`,
    {
      headers: { Authorization: `Bearer ${token}` },
      data: { filename: fileName, file_size: content.length, mime_type: mimeType },
    },
  );
  expect(createResp.ok(), `create document failed: ${await createResp.text()}`).toBeTruthy();
  const createBody = (await createResp.json()) as {
    document_id?: string;
    upload_url?: string;
  };
  const documentId = String(createBody?.document_id || "");
  expect(documentId, "document_id was empty").not.toBe("");

  // Step 2: Upload file bytes to dev-upload endpoint
  const uploadResp = await request.put(`/dev-upload/${documentId}`, {
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/octet-stream",
    },
    data: content,
  });
  expect(uploadResp.ok(), `upload failed: ${await uploadResp.text()}`).toBeTruthy();

  // Step 3: Wait for ingestion to complete
  const attempts = Math.max(1, Math.ceil(maxWaitMs / 1000));
  for (let i = 0; i < attempts; i++) {
    const statusResp = await request.get(
      `/api/v1/documents/${documentId}/status`,
      { headers: { Authorization: `Bearer ${token}` } },
    );
    if (statusResp.ok()) {
      const statusBody = (await statusResp.json()) as { status?: string };
      if (statusBody.status === "completed") return documentId;
      if (statusBody.status === "failed") {
        throw new Error(`Document ingestion failed for ${documentId}`);
      }
    }
    await new Promise((r) => setTimeout(r, 1000));
  }

  throw new Error(
    `Document ${documentId} did not reach completed state within ${Math.floor(maxWaitMs / 1000)}s`,
  );
}

export interface SSEEvent {
  event: string;
  data: string;
}

export async function collectSSEEvents(
  request: APIRequestContext,
  token: string,
  chatBody: Record<string, unknown>,
  requestId = uniqueName("req"),
): Promise<SSEEvent[]> {
  const response = await request.post("/api/v1/chat", {
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      Accept: "text/event-stream",
      "x-request-id": requestId,
    },
    data: {
      ...chatBody,
      stream: true,
    },
    maxRedirects: 0,
  });

  expect(response.ok(), `chat SSE failed: ${response.status()}`).toBeTruthy();

  const body = await response.text();
  const events: SSEEvent[] = [];
  let currentEvent = "message";
  let currentData = "";

  for (const line of body.split("\n")) {
    if (line.startsWith("event: ")) {
      currentEvent = line.slice(7).trim();
    } else if (line.startsWith("data: ")) {
      currentData = line.slice(6);
    } else if (line === "" && currentData) {
      events.push({ event: currentEvent, data: currentData });
      currentEvent = "message";
      currentData = "";
    }
  }

  return events;
}

export function authHeaders(token: string): Record<string, string> {
  return { Authorization: `Bearer ${token}` };
}

export async function seedBrowserAuth(
  page: Page,
  request: APIRequestContext,
  token: string,
): Promise<void> {
  const me = await request.get("/api/auth/me", {
    headers: authHeaders(token),
  });
  expect(me.ok(), `/me failed while seeding browser auth: ${me.status()}`).toBeTruthy();
  const payload = (await me.json()) as {
    data?: { user?: { id?: string; email?: string; full_name?: string } };
  };
  const user = payload?.data?.user;
  expect(user?.id, "seedBrowserAuth missing user id").toBeTruthy();
  expect(user?.email, "seedBrowserAuth missing user email").toBeTruthy();
  expect(user?.full_name, "seedBrowserAuth missing full name").toBeTruthy();

  await page.addInitScript((authPayload) => {
    window.localStorage.setItem("avrag.auth.v1", JSON.stringify(authPayload));
  }, {
    token,
    user: {
      id: user!.id!,
      email: user!.email!,
      full_name: user!.full_name!,
    },
  });
}

export async function waitForHydration(page: Page, timeout = 15_000): Promise<void> {
  await expect(page.locator("html")).toHaveAttribute("data-hydrated", "true", { timeout });
}

export async function gotoAndWaitForHydration(
  page: Page,
  url: string,
  timeout = 15_000,
): Promise<void> {
  await page.goto(url);
  await waitForHydration(page, timeout);
}
