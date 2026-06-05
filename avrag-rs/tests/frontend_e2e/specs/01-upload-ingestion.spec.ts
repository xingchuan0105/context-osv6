import { test, expect } from "@playwright/test";

test.describe("Upload & Ingestion", () => {
  test("file upload triggers ingestion progress", async ({ page, request }) => {
    const backendUrl = process.env.BACKEND_BASE_URL || "http://127.0.0.1:8080";
    const notebookRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "upload-test", description: "" },
    });
    expect(notebookRes.status()).toBe(201);
    const notebook = await notebookRes.json();

    // Upload via API (UI drag-drop can be flaky in headless)
    const uploadInit = await request.post(
      `${backendUrl}/api/v1/notebooks/${notebook.notebook.id}/documents`,
      { data: { filename: "antifragile.txt", file_size: 1728, mime_type: "text/plain" } }
    );
    expect(uploadInit.status()).toBe(200);
    const { document_id: docId } = await uploadInit.json();

    const fileContent = await import("fs").then((fs) =>
      fs.promises.readFile("../fixtures/documents/antifragile.txt", "utf-8")
    );
    const putRes = await request.put(`${backendUrl}/dev-upload/${docId}`, { data: fileContent });
    expect(putRes.status()).toBe(200);

    // Poll until completed
    let status = "queued";
    const deadline = Date.now() + 180_000;
    while (status !== "completed" && Date.now() < deadline) {
      const s = await request.get(`${backendUrl}/api/v1/documents/${docId}/status`);
      const body = await s.json();
      status = body.status;
      if (status === "failed") throw new Error("Ingestion failed");
      await new Promise((r) => setTimeout(r, 500));
    }
    expect(status).toBe("completed");
  });
});
