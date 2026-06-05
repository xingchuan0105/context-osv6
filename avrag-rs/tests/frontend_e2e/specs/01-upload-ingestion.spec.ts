import { test, expect } from "@playwright/test";
import { getBackendBaseUrl, defaultAuthHeaders } from "../src/setup/backendUrl";

test.describe("Upload & Ingestion", () => {
  test("file upload triggers ingestion progress", async ({ page, request }) => {
    const backendUrl = getBackendBaseUrl();
    const headers = defaultAuthHeaders();
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "upload-test", description: "" },
      headers,
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    // Upload via API (UI drag-drop can be flaky in headless)
    const uploadInit = await request.post(
      `${backendUrl}/api/v1/notebooks/${notebook.notebook.id}/documents`,
      { data: { filename: "antifragile.txt", file_size: 1728, mime_type: "text/plain" }, headers }
    );
    expect(uploadInit.status()).toBe(201);
    const { document_id: docId } = await uploadInit.json();

    const fixturePath = require("path").resolve(__dirname, "../fixtures/documents/antifragile.txt");
    const fileContent = await require("fs").promises.readFile(fixturePath, "utf-8");
    const putRes = await request.put(`${backendUrl}/dev-upload/${docId}`, { data: fileContent, headers });
    expect(putRes.status()).toBe(200);

    // Poll until completed
    let status = "queued";
    const deadline = Date.now() + 180_000;
    while (status !== "completed" && Date.now() < deadline) {
      const s = await request.get(`${backendUrl}/api/v1/documents/${docId}/status`, { headers });
      const body = await s.json();
      status = body.status;
      if (status === "failed") throw new Error("Ingestion failed");
      await new Promise((r) => setTimeout(r, 500));
    }
    expect(status).toBe("completed");
  });
});
