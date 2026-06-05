import { test, expect } from "@playwright/test";

test.describe("Notebook CRUD", () => {
  test("create rename and delete notebook", async ({ page, request }) => {
    const backendUrl = process.env.BACKEND_BASE_URL || "http://127.0.0.1:8080";

    // Create
    const createRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "crud-test", description: "" },
    });
    expect(createRes.status()).toBe(201);
    const nb = (await createRes.json()).notebook;

    // Rename
    const renameRes = await request.patch(`${backendUrl}/api/v1/notebooks/${nb.id}`, {
      data: { name: "crud-test-renamed" },
    });
    expect(renameRes.status()).toBe(200);

    // List and verify
    const listRes = await request.get(`${backendUrl}/api/v1/notebooks`);
    const list = await listRes.json();
    const found = list.notebooks.find((n: any) => n.id === nb.id);
    expect(found?.name).toBe("crud-test-renamed");

    // Delete
    const delRes = await request.delete(`${backendUrl}/api/v1/notebooks/${nb.id}`);
    expect(delRes.status()).toBe(200);
  });
});
