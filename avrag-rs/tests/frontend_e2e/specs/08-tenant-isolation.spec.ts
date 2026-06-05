import { test, expect } from "@playwright/test";

const ORG_A = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const USER_A = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const ORG_B = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
const USER_B = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

test.describe("Tenant isolation", () => {
  test("org-b cannot see org-a documents", async ({ request }) => {
    const backendUrl = process.env.BACKEND_BASE_URL || "http://127.0.0.1:8080";

    // Org-A uploads
    const nbA = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "iso-test-a", description: "" },
      headers: { "x-org-id": ORG_A, "x-user-id": USER_A },
    });
    const notebookA = (await nbA.json()).notebook;

    const uploadA = await request.post(
      `${backendUrl}/api/v1/notebooks/${notebookA.id}/documents`,
      { data: { filename: "antifragile.txt", file_size: 1728, mime_type: "text/plain" }, headers: { "x-org-id": ORG_A, "x-user-id": USER_A } }
    );
    const docA = (await uploadA.json()).document_id;
    const fileContent = await import("fs").then((fs) => fs.promises.readFile("../fixtures/documents/antifragile.txt", "utf-8"));
    await request.put(`${backendUrl}/dev-upload/${docA}`, { data: fileContent, headers: { "x-org-id": ORG_A, "x-user-id": USER_A } });

    // Org-B queries with Org-A doc_id
    const nbB = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "iso-test-b", description: "" },
      headers: { "x-org-id": ORG_B, "x-user-id": USER_B },
    });
    const notebookB = (await nbB.json()).notebook;

    const chatRes = await request.post(`${backendUrl}/api/v1/chat`, {
      data: {
        query: "What is antifragility?",
        agent_type: "rag",
        notebook_id: notebookB.id,
        doc_scope: [docA],
        stream: false,
      },
      headers: { "x-org-id": ORG_B, "x-user-id": USER_B, "x-permissions": "external_network" },
    });

    const body = await chatRes.json();
    const leaked = (body.citations || []).some((c: any) => c.doc_id === docA);
    expect(leaked).toBe(false);
  });
});
