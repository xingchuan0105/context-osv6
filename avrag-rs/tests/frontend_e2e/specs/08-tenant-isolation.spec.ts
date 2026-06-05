import { test, expect } from "@playwright/test";
import { getBackendBaseUrl } from "../src/setup/backendUrl";
function isoHeaders(org: string, user: string): Record<string, string> {
  return {
    "x-org-id": org,
    "x-user-id": user,
    "x-permissions": "external_network",
  };
}

const ORG_A = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const USER_A = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const ORG_B = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
const USER_B = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

test.describe("Tenant isolation", () => {
  test("org-b cannot see org-a documents", async ({ request }) => {
    const backendUrl = getBackendBaseUrl();

    // Org-A creates notebook
    const nbA = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "iso-test-a", description: "" },
      headers: isoHeaders(ORG_A, USER_A),
    });
    expect(nbA.status(), `notebook create failed: ${await nbA.text()}`).toBe(201);
    const notebookA = (await nbA.json()).notebook;
    expect(notebookA, "notebookA missing from response").toBeTruthy();

    // Org-A uploads document
    const uploadA = await request.post(
      `${backendUrl}/api/v1/notebooks/${notebookA.id}/documents`,
      { data: { filename: "antifragile.txt", file_size: 1728, mime_type: "text/plain" }, headers: isoHeaders(ORG_A, USER_A) }
    );
    expect(uploadA.status(), `upload failed: ${await uploadA.text()}`).toBe(201);
    const docA = (await uploadA.json()).document_id;

    const fixturePath = require("path").resolve(__dirname, "../fixtures/documents/antifragile.txt");
    const fileContent = await require("fs").promises.readFile(fixturePath, "utf-8");
    const putA = await request.put(`${backendUrl}/dev-upload/${docA}`, { data: fileContent, headers: isoHeaders(ORG_A, USER_A) });
    expect(putA.status(), `file put failed: ${await putA.text()}`).toBe(200);

    // Org-B creates notebook
    const nbB = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "iso-test-b", description: "" },
      headers: isoHeaders(ORG_B, USER_B),
    });
    expect(nbB.status(), `notebookB create failed: ${await nbB.text()}`).toBe(201);
    const notebookB = (await nbB.json()).notebook;

    // Org-B queries with Org-A doc_id — should not leak
    const chatRes = await request.post(`${backendUrl}/api/v1/chat`, {
      data: {
        query: "What is antifragility?",
        agent_type: "rag",
        notebook_id: notebookB.id,
        doc_scope: [docA],
        stream: false,
      },
      headers: isoHeaders(ORG_B, USER_B),
    });

    expect(chatRes.status(), `chat failed: ${await chatRes.text()}`).toBe(200);
    const body = await chatRes.json();
    const leaked = (body.citations || []).some((c: any) => c.doc_id === docA);
    expect(leaked).toBe(false);
  });
});
