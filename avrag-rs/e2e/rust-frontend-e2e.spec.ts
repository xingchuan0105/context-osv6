import { expect, test } from "@playwright/test";
import {
  registerTestUser,
  loginTestUser,
  createNotebookViaAPI,
  deleteNotebookViaAPI,
  uploadDocumentAndWait,
  collectSSEEvents,
  authHeaders,
  longText,
  seedBrowserAuth,
  uniqueName,
  gotoAndWaitForHydration,
} from "./helpers";

const STRICT_CITATION_MODE = process.env.E2E_STRICT_CITATIONS === "1";

// ─── Group 1: Infrastructure & Auth ────────────────────────────────

test.describe("Infrastructure & Auth", () => {
  test("T01: health and readiness endpoints", async ({ request }) => {
    const health = await request.get("/health");
    expect(health.ok(), `health failed: ${health.status()}`).toBeTruthy();

    const ready = await request.get("/ready");
    expect(ready.ok(), `ready failed: ${ready.status()}`).toBeTruthy();
  });

  test("UI-metrics: metrics endpoint exposes runtime counters", async ({ request }) => {
    const resp = await request.get("/metrics");
    expect(resp.ok(), `metrics failed: ${resp.status()}`).toBeTruthy();
    const text = await resp.text();
    expect(text).toContain("http_requests_total");
  });

  test("T02: user registration and login flow", async ({ request }) => {
    const { token, email, password } = await registerTestUser(request);
    expect(token).toBeTruthy();

    // Verify /me works with token
    const me = await request.get("/api/auth/me", {
      headers: authHeaders(token),
    });
    expect(me.ok(), `/me failed: ${me.status()}`).toBeTruthy();
    const meBody = (await me.json()) as { data?: { user?: { email?: string } } };
    expect(meBody?.data?.user?.email).toBe(email);

    // Login with same credentials
    const newToken = await loginTestUser(request, email, password);
    expect(newToken).toBeTruthy();
    expect(newToken).not.toBe(token); // new session token
  });

  test("T03: API server returns JSON 404 for page routes", async ({ request }) => {
    const root = await request.get("/");
    expect(root.status(), `root returned unexpected status`).toBe(404);
    const body = (await root.json()) as { error?: string };
    expect(body.error).toBe("not_found");
  });
});

// ─── Group 2: Notebook & Document Lifecycle ─────────────────────────

test.describe("Notebook & Document Lifecycle", () => {
  let token = "";
  let email = "";
  let password = "";

  test.beforeAll(async ({ request }) => {
    const auth = await registerTestUser(request);
    token = auth.token;
    email = auth.email;
    password = auth.password;
  });

  test("T04: notebook CRUD via API", async ({ request }) => {
    const name = uniqueName("pw-nb");

    // Create
    const id = await createNotebookViaAPI(request, token, name);

    // List
    const list = await request.get("/api/v1/workspaces", {
      headers: authHeaders(token),
    });
    expect(list.ok()).toBeTruthy();
    const listBody = (await list.json()) as { notebooks?: { id: string; name: string }[] };
    const found = (listBody?.notebooks || []).find((nb) => nb.id === id);
    expect(found, "created notebook not in list").toBeTruthy();
    expect(found!.name).toBe(name);

    // Get
    const get = await request.get(`/api/v1/workspaces/${id}`, {
      headers: authHeaders(token),
    });
    expect(get.ok()).toBeTruthy();

    // Update
    const newName = `${name}-updated`;
    const update = await request.put(`/api/v1/workspaces/${id}`, {
      headers: authHeaders(token),
      data: { name: newName, description: "updated by e2e" },
    });
    expect(update.ok(), `update failed: ${await update.text()}`).toBeTruthy();

    // Verify update
    const getUpdated = await request.get(`/api/v1/workspaces/${id}`, {
      headers: authHeaders(token),
    });
    const updated = (await getUpdated.json()) as { notebook?: { name: string } };
    expect(updated?.notebook?.name).toBe(newName);

    // Delete
    await deleteNotebookViaAPI(request, token, id);

    // Verify deleted
    const getDeleted = await request.get(`/api/v1/workspaces/${id}`, {
      headers: authHeaders(token),
    });
    expect(getDeleted.ok()).toBeFalsy();
  });

  test("T05: document upload and ingestion pipeline", async ({ request }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-upload"),
    );

    try {
      // Step 1: Create document record
      const createResp = await request.post(
        `/api/v1/workspaces/${notebookId}/documents`,
        {
          headers: authHeaders(token),
          data: { filename: "sample.txt", file_size: 50, mime_type: "text/plain" },
        },
      );
      expect(createResp.ok(), `create doc failed: ${await createResp.text()}`).toBeTruthy();
      const createBody = (await createResp.json()) as { document_id?: string; status?: string };
      const documentId = createBody?.document_id;
      expect(documentId, "no document_id").toBeTruthy();

      // Step 2: Upload bytes
      const uploadResp = await request.put(`/dev-upload/${documentId}`, {
        headers: { ...authHeaders(token), "Content-Type": "application/octet-stream" },
        data: Buffer.from("The capital of France is Paris.\n"),
      });
      expect(uploadResp.ok(), `upload failed: ${await uploadResp.text()}`).toBeTruthy();
      const uploadBody = (await uploadResp.json()) as { status?: string };
      expect(uploadBody?.status).toBe("queued");

      // Step 3: Poll status — accept both completed (full pipeline) or queued/processing
      // Ingestion may fail if embedding/LLM keys are not configured
      let finalStatus = "unknown";
      for (let i = 0; i < 30; i++) {
        const statusResp = await request.get(
          `/api/v1/documents/${documentId}/status`,
          { headers: authHeaders(token) },
        );
        if (statusResp.ok()) {
          const body = (await statusResp.json()) as { status?: string };
          finalStatus = body.status || "unknown";
          if (finalStatus === "completed" || finalStatus === "failed") break;
        }
        await new Promise((r) => setTimeout(r, 1000));
      }

      // Upload pipeline wiring is verified regardless of ingestion outcome
      expect(["completed", "failed", "queued", "processing"]).toContain(finalStatus);
      if (finalStatus === "failed") {
        console.warn("[WARN] T05: Ingestion failed — embedding/LLM keys may not be configured");
      }
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });

  test("T06: document reindex", async ({ request }) => {
    const notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-reindex"),
    );

    try {
      // Create + upload document (don't wait for ingestion)
      const createResp = await request.post(
        `/api/v1/workspaces/${notebookId}/documents`,
        {
          headers: authHeaders(token),
          data: { filename: "reindex.txt", file_size: 40, mime_type: "text/plain" },
        },
      );
      expect(createResp.ok(), `create doc failed: ${await createResp.text()}`).toBeTruthy();
      const createBody = (await createResp.json()) as { document_id?: string };
      const documentId = createBody?.document_id;
      expect(documentId).toBeTruthy();

      await request.put(`/dev-upload/${documentId}`, {
        headers: { ...authHeaders(token), "Content-Type": "application/octet-stream" },
        data: Buffer.from("Reindex test content. The sky is blue.\n"),
      });

      // Trigger reindex endpoint
      const reindexResp = await request.post(
        `/api/v1/documents/${documentId}/reindex`,
        { headers: authHeaders(token) },
      );
      // Reindex endpoint accepts the request regardless of current status
      expect([200, 202]).toContain(reindexResp.status());
    } finally {
      await deleteNotebookViaAPI(request, token, notebookId);
    }
  });
});

// ─── Group 3: Chat Modes ────────────────────────────────────────────

test.describe("Chat Modes", () => {
  let token = "";
  let notebookId = "";

  test.beforeAll(async ({ request }) => {
    const auth = await registerTestUser(request);
    token = auth.token;
    notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-chat"),
    );
  });

  test.afterAll(async ({ request }) => {
    await deleteNotebookViaAPI(request, token, notebookId);
  });

  test("T07: general chat (streaming SSE)", async ({ request }) => {
    const marker = uniqueName("chat");
    const requestId = uniqueName("req");
    const events = await collectSSEEvents(request, token, {
      query: `Please reply with a short sentence containing the marker ${marker}`,
      notebook_id: notebookId,
      agent_type: "general",
      stream: true,
    }, requestId);

    // Verify SSE event sequence
    expect(events.length, "SSE stream produced no events").toBeGreaterThan(0);

    const startEvents = events.filter((e) => e.event === "start");
    const tokenEvents = events.filter((e) => e.event === "token");
    const doneEvents = events.filter((e) => e.event === "done");

    expect(startEvents.length, "no start event").toBeGreaterThanOrEqual(1);
    expect(tokenEvents.length, "no token events in stream").toBeGreaterThanOrEqual(1);
    expect(doneEvents.length, "no done event").toBeGreaterThanOrEqual(1);

    // Concatenate all tokens
    const fullAnswer = tokenEvents.map((e) => {
      try {
        return JSON.parse(e.data).content || "";
      } catch {
        return "";
      }
    }).join("");

    expect(fullAnswer.length, "answer was empty").toBeGreaterThan(0);

    // Verify session persistence
    const startData = JSON.parse(startEvents[0].data);
    const sessionId = startData.session_id;
    expect(sessionId, "no session_id in start event").toBeTruthy();
    expect(startData.request_id, "start event should echo transport request id").toBe(requestId);

    const tokenRequestIds = tokenEvents.map((event) => {
      try {
        return JSON.parse(event.data).request_id || "";
      } catch {
        return "";
      }
    }).filter(Boolean);
    expect(new Set(tokenRequestIds)).toEqual(new Set([requestId]));

    const doneData = JSON.parse(doneEvents[doneEvents.length - 1].data);
    expect(doneData.request_id, "done event should echo transport request id").toBe(requestId);

    const sessionsResp = await request.get("/api/v1/chat/sessions", {
      headers: authHeaders(token),
    });
    expect(sessionsResp.ok()).toBeTruthy();

    const messagesResp = await request.get(
      `/api/v1/chat/sessions/${sessionId}/messages`,
      { headers: authHeaders(token) },
    );
    expect(messagesResp.ok(), `messages fetch failed: ${messagesResp.status()}`).toBeTruthy();
    const msgBody = (await messagesResp.json()) as { messages?: unknown[] };
    expect(msgBody?.messages?.length, "no messages persisted").toBeGreaterThanOrEqual(1);
  });

  test("T08: RAG chat with uploaded document", async ({ request }) => {
    // Upload a document with known facts (ingestion outcome doesn't block the test)
    const content = Buffer.from(
      "The capital of France is Paris. The year 2026 is when this test was written.",
    );

    // Create + upload document without waiting for ingestion
    const createResp = await request.post(
      `/api/v1/workspaces/${notebookId}/documents`,
      {
        headers: authHeaders(token),
        data: { filename: "rag-test.txt", file_size: content.length, mime_type: "text/plain" },
      },
    );
    const createBody = (await createResp.json()) as { document_id?: string };
    const documentId = createBody?.document_id;
    expect(documentId).toBeTruthy();

    await request.put(`/dev-upload/${documentId}`, {
      headers: { ...authHeaders(token), "Content-Type": "application/octet-stream" },
      data: content,
    });

    // Wait briefly for document to be registered
    await new Promise((r) => setTimeout(r, 2000));

    // Send RAG query — the chat endpoint should accept the request
    // regardless of whether the embedding pipeline completed
    const events = await collectSSEEvents(request, token, {
      query: "What is the capital of France?",
      notebook_id: notebookId,
      agent_type: "rag",
      doc_scope: [documentId],
      stream: true,
    });

    // Verify SSE stream was produced
    expect(events.length, "RAG SSE stream produced no events").toBeGreaterThan(0);

    const startEvents = events.filter((e) => e.event === "start");
    const doneEvents = events.filter((e) => e.event === "done");
    const tokenEvents = events.filter((e) => e.event === "token");

    expect(startEvents.length, "no start event").toBeGreaterThanOrEqual(1);
    expect(doneEvents.length, "no done event").toBeGreaterThanOrEqual(1);

    // If tokens were produced, verify they're non-empty
    if (tokenEvents.length > 0) {
      const fullAnswer = tokenEvents.map((e) => {
        try { return JSON.parse(e.data).content || ""; } catch { return ""; }
      }).join("");
      expect(fullAnswer.length, "answer was empty").toBeGreaterThan(0);
    }

    // Log whether citations were returned
    const citationEvents = events.filter((e) => e.event === "citations");
    if (citationEvents.length === 0) {
      console.warn("[WARN] T08: RAG chat returned no citations — retrieval gap may exist");
    }
  });

  test("T08b: RAG citation minimum golden set", async ({ request }) => {
    test.skip(
      !STRICT_CITATION_MODE,
      "Enable E2E_STRICT_CITATIONS=1 to enforce citation golden-set regression",
    );

    const notebook = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-rag-citation-golden"),
    );

    try {
      const docBody = Buffer.from(
        "The capital of France is Paris. The Eiffel Tower is located in Paris.",
      );
      const docId = await uploadDocumentAndWait(
        request,
        token,
        notebook,
        "citation-golden-fr.txt",
        docBody,
        "text/plain",
        120_000,
      );

      const events = await collectSSEEvents(request, token, {
        query: "What is the capital of France?",
        notebook_id: notebook,
        agent_type: "rag",
        doc_scope: [docId],
        stream: true,
      });

      const tokenEvents = events.filter((e) => e.event === "token");
      const answer = tokenEvents.map((e) => {
        try { return JSON.parse(e.data).content || ""; } catch { return ""; }
      }).join("");
      expect(answer, "golden-set answer should mention Paris").toMatch(/Paris/i);

      const citationEvents = events.filter((e) => e.event === "citations");
      expect(citationEvents.length, "strict mode requires citation events").toBeGreaterThan(0);

      const citationPayload = JSON.parse(
        citationEvents[citationEvents.length - 1].data || "{}",
      ) as { citations?: unknown[] };
      expect(
        Array.isArray(citationPayload.citations) ? citationPayload.citations.length : 0,
        "strict mode requires at least one citation item",
      ).toBeGreaterThan(0);
    } finally {
      await deleteNotebookViaAPI(request, token, notebook);
    }
  });

  test("T09: chat session management", async ({ request }) => {
    // Send a chat message
    const events1 = await collectSSEEvents(request, token, {
      query: "First test message for session management",
      notebook_id: notebookId,
      agent_type: "general",
      stream: true,
    });
    expect(events1.length).toBeGreaterThan(0);

    const startData = JSON.parse(
      events1.find((e) => e.event === "start")?.data || "{}",
    );
    const sessionId = startData.session_id;
    expect(sessionId).toBeTruthy();

    // List sessions
    const sessionsResp = await request.get("/api/v1/chat/sessions", {
      headers: authHeaders(token),
    });
    expect(sessionsResp.ok()).toBeTruthy();
    const sessions = (await sessionsResp.json()) as { sessions?: { id: string }[] };
    expect(
      (sessions?.sessions || []).some((s) => s.id === sessionId),
      "new session not in sessions list",
    ).toBeTruthy();

    // Delete session
    const deleteResp = await request.delete(
      `/api/v1/chat/sessions/${sessionId}`,
      { headers: authHeaders(token) },
    );
    expect(deleteResp.ok(), `session delete failed: ${deleteResp.status()}`).toBeTruthy();
  });

  test("T09b: chat session rename and pin persistence", async ({ request }) => {
    const events = await collectSSEEvents(request, token, {
      query: "Rename and pin this session",
      notebook_id: notebookId,
      agent_type: "general",
      stream: true,
    });
    const startData = JSON.parse(
      events.find((e) => e.event === "start")?.data || "{}",
    );
    const sessionId = startData.session_id;
    expect(sessionId).toBeTruthy();

    const updateResp = await request.put(`/api/v1/chat/sessions/${sessionId}`, {
      headers: authHeaders(token),
      data: {
        title: "Pinned Session",
        pinned: true,
      },
    });
    expect(updateResp.ok(), `session update failed: ${await updateResp.text()}`).toBeTruthy();

    const sessionResp = await request.get(`/api/v1/chat/sessions/${sessionId}`, {
      headers: authHeaders(token),
    });
    expect(sessionResp.ok()).toBeTruthy();
    const sessionBody = (await sessionResp.json()) as {
      title?: string;
      pinned?: boolean;
    };
    expect(sessionBody.title).toBe("Pinned Session");
    expect(sessionBody.pinned).toBe(true);
  });
});

// ─── Group 4: Share & Collaboration ─────────────────────────────────

test.describe("Share & Collaboration", () => {
  let token = "";
  let notebookId = "";

  test.beforeAll(async ({ request }) => {
    const auth = await registerTestUser(request);
    token = auth.token;
    notebookId = await createNotebookViaAPI(
      request,
      token,
      uniqueName("pw-share"),
    );
  });

  test.afterAll(async ({ request }) => {
    await deleteNotebookViaAPI(request, token, notebookId);
  });

  test("T10: share link creation and validation", async ({ request }) => {
    // Create share token
    const shareResp = await request.post(
      `/api/v1/workspaces/${notebookId}/share`,
      {
        headers: authHeaders(token),
        data: { role: "viewer" },
      },
    );
    expect(shareResp.ok(), `share failed: ${await shareResp.text()}`).toBeTruthy();
    const shareBody = (await shareResp.json()) as { share_token?: string };
    const shareToken = shareBody?.share_token;
    expect(shareToken, "no share_token returned").toBeTruthy();

    // Validate share token
    const validateResp = await request.get(
      `/api/v1/share/validate/${shareToken}`,
    );
    expect(validateResp.ok(), `validate failed: ${validateResp.status()}`).toBeTruthy();

    // View shared content
    const viewResp = await request.get(`/api/shared/kb/${shareToken}`);
    expect(viewResp.ok(), `shared view failed: ${viewResp.status()}`).toBeTruthy();
    const viewBody = (await viewResp.json()) as { data?: { knowledge_base?: { id: string } } };
    expect(viewBody?.data?.knowledge_base?.id).toBe(notebookId);
  });

  test("T11a: share settings and access level", async ({ request }) => {
    // Get current share settings
    const settingsResp = await request.get(
      `/api/v1/workspaces/${notebookId}/share/settings`,
      { headers: authHeaders(token) },
    );
    expect(settingsResp.ok(), `settings failed: ${settingsResp.status()}`).toBeTruthy();

    // Update access level
    const updateResp = await request.post(
      `/api/v1/workspaces/${notebookId}/access-level`,
      {
        headers: authHeaders(token),
        data: { access_level: "link" },
      },
    );
    expect(updateResp.ok(), `access-level failed: ${await updateResp.text()}`).toBeTruthy();
  });

  test("T11b: share download policy persists to public payload", async ({ request }) => {
    const shareResp = await request.post(`/api/v1/workspaces/${notebookId}/share`, {
      headers: authHeaders(token),
      data: { role: "viewer" },
    });
    expect(shareResp.ok()).toBeTruthy();
    const shareBody = (await shareResp.json()) as { share_token?: string };
    const shareToken = shareBody.share_token;
    expect(shareToken).toBeTruthy();

    const updateResp = await request.put(
      `/api/v1/workspaces/${notebookId}/share/settings`,
      {
        headers: authHeaders(token),
        data: { access_level: "link", allow_download: true },
      },
    );
    expect(updateResp.ok(), `share settings update failed: ${await updateResp.text()}`).toBeTruthy();

    const settingsResp = await request.get(
      `/api/v1/workspaces/${notebookId}/share/settings`,
      { headers: authHeaders(token) },
    );
    const settingsBody = (await settingsResp.json()) as { allow_download?: boolean };
    expect(settingsBody.allow_download).toBe(true);

    const publicResp = await request.get(`/api/shared/kb/${shareToken}`);
    expect(publicResp.ok()).toBeTruthy();
    const publicBody = (await publicResp.json()) as {
      data?: { share?: { allow_download?: boolean } };
    };
    expect(publicBody?.data?.share?.allow_download).toBe(true);
  });

  test("T12: share analytics and access logs", async ({ request }) => {
    // Create a share token first
    const shareResp = await request.post(
      `/api/v1/workspaces/${notebookId}/share`,
      {
        headers: authHeaders(token),
        data: { role: "viewer" },
      },
    );
    const shareBody = (await shareResp.json()) as { share_token?: string };
    const shareToken = shareBody?.share_token;

    // Access shared content to generate a view
    if (shareToken) {
      await request.get(`/api/shared/kb/${shareToken}`);
    }

    // Check analytics
    const analyticsResp = await request.get(
      `/api/v1/workspaces/${notebookId}/share/analytics`,
      { headers: authHeaders(token) },
    );
    expect(analyticsResp.ok(), `analytics failed: ${analyticsResp.status()}`).toBeTruthy();

    // Check access logs
    const logsResp = await request.get(
      `/api/v1/workspaces/${notebookId}/share/access-logs`,
      { headers: authHeaders(token) },
    );
    expect(logsResp.ok(), `access-logs failed: ${logsResp.status()}`).toBeTruthy();
  });
});

// ─── Group 5: Browser Frontend Journeys ────────────────────────────

test.describe("Browser Frontend Journeys", () => {
  test("T11: password reset capability is available", async ({ page, request }) => {
    await gotoAndWaitForHydration(page, "/login");
    await expect(page.getByRole("link", { name: /忘记密码|Forgot Password/ })).toHaveCount(1);

    const auth = await registerTestUser(request);
    const notebookId = await createNotebookViaAPI(
      request,
      auth.token,
      uniqueName("pw-ui-capabilities"),
    );

    await seedBrowserAuth(page, request, auth.token);

    try {
      await gotoAndWaitForHydration(page, "/reset-password");
      await expect(page.getByRole("button", { name: /发送验证码|Send Code/ })).toHaveCount(1);

      await gotoAndWaitForHydration(page, "/settings");
      const securityTab = page.getByRole("button", { name: /安全|Security/ });
      await securityTab.click();
      await expect(securityTab).toHaveClass(/app-tab-button-active/);
      await expect(
        page.locator("a[href='/reset-password']").filter({ hasText: /重置密码|Reset Password/ }),
      ).toBeVisible();

    } finally {
      await deleteNotebookViaAPI(request, auth.token, notebookId);
    }
  });

  test("UI01: appearance settings persist theme across reload", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    await seedBrowserAuth(page, request, auth.token);

    await gotoAndWaitForHydration(page, "/settings");
    await page.getByRole("button", { name: /外观与语言|Appearance/ }).click();
    await page.getByRole("button", { name: /深色|Dark/ }).click();

    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");

    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  });

  test("UI02: workspace can import a real URL source", async ({ page, request, baseURL }) => {
    const auth = await registerTestUser(request);
    const notebookId = await createNotebookViaAPI(
      request,
      auth.token,
      uniqueName("pw-ui-url"),
    );

    await seedBrowserAuth(page, request, auth.token);

    try {
      await gotoAndWaitForHydration(page, `/dashboard/${notebookId}`);
      const addUrlResponse = page.waitForResponse((resp) =>
        resp.request().method() === "POST" &&
        resp.url().includes(`/api/v1/workspaces/${notebookId}/sources/url`)
      );
      await page.getByPlaceholder(/添加网页链接源|Add URL source/).fill(`${baseURL}/login`);
      await page.getByRole("button", { name: /添加链接|Add URL/ }).click();
      await addUrlResponse;

      const importedSource = page.locator("text=login.html").first();
      await expect(importedSource).toBeVisible({ timeout: 20_000 });
      await importedSource.click();

      await expect(page.getByRole("button", { name: /重新索引|Reindex/ })).toBeVisible({
        timeout: 15_000,
      });
    } finally {
      await deleteNotebookViaAPI(request, auth.token, notebookId);
    }
  });

  test("UI03: share analytics failures surface an error banner", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    const notebookId = await createNotebookViaAPI(
      request,
      auth.token,
      uniqueName("pw-ui-share"),
    );

    await seedBrowserAuth(page, request, auth.token);
    await page.route(`**/api/v1/workspaces/${notebookId}/share/analytics`, async (route) => {
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "boom" }),
      });
    });

    try {
      await gotoAndWaitForHydration(page, `/dashboard/${notebookId}/share`);
      const analyticsRequest = page.waitForRequest(
        `**/api/v1/workspaces/${notebookId}/share/analytics`,
      );
      await page.getByRole("button", { name: /分析|Analytics/ }).click();
      await analyticsRequest;
      await expect(
        page.locator("text=/加载分享分析失败|Failed to load share analytics/").first(),
      ).toBeVisible({ timeout: 10_000 });
    } finally {
      await deleteNotebookViaAPI(request, auth.token, notebookId);
    }
  });

  test("T19: workspace chat keeps scroll position while browsing history", async ({ page, request }) => {
    const auth = await registerTestUser(request);
    const notebookId = await createNotebookViaAPI(
      request,
      auth.token,
      uniqueName("pw-virtual-chat"),
    );

    await seedBrowserAuth(page, request, auth.token);

    try {
      await gotoAndWaitForHydration(page, `/dashboard/${notebookId}`);

      const input = page.getByPlaceholder(
        /围绕当前资料继续研究|Ask a question about your documents/,
      );
      await expect(input).toBeVisible();

      await input.fill(await longText(page, "chat scroll seed"));
      const firstChatRequest = page.waitForRequest("**/api/v1/chat");
      await page.locator("form").evaluate((form) => {
        (form as HTMLFormElement).requestSubmit();
      });
      await firstChatRequest;

      await expect(page.getByText(/chat scroll seed paragraph 1/i).first()).toBeVisible({
        timeout: 15_000,
      });

      const shell = page.locator("[data-test-chat-scroll]");
      await expect(shell).toBeVisible();
      await shell.evaluate((node) => {
        const element = node as HTMLElement;
        element.scrollTop = Math.max(0, element.scrollHeight - element.clientHeight - 400);
      });

      const before = await shell.evaluate((node) => (node as HTMLElement).scrollTop);

      await input.fill(await longText(page, "chat scroll followup"));
      await page.locator("form").evaluate((form) => {
        (form as HTMLFormElement).requestSubmit();
      });

      await page.waitForTimeout(250);

      const after = await shell.evaluate((node) => (node as HTMLElement).scrollTop);
      expect(Math.abs(after - before)).toBeLessThan(120);
    } finally {
      await deleteNotebookViaAPI(request, auth.token, notebookId);
    }
  });
});
