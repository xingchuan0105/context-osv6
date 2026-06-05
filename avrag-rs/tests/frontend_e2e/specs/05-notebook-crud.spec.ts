import { test, expect } from "@playwright/test";
import { getBackendBaseUrl, defaultAuthHeaders } from "../src/setup/backendUrl";

test.describe("Notebook CRUD", () => {
  test("create rename and delete notebook", async ({ page, request }) => {
    const backendUrl = getBackendBaseUrl();
    const headers = defaultAuthHeaders();

    // Create
    const createRes = await request.post(`${backendUrl}/api/v1/notebooks`, {
      data: { name: "crud-test", description: "" },
      headers,
    });
    expect(createRes.status()).toBe(201);
    const nb = (await createRes.json()).notebook;

    // Rename
    const renameRes = await request.patch(`${backendUrl}/api/v1/notebooks/${nb.id}`, {
      data: { name: "crud-test-renamed" },
      headers,
    });
    expect(renameRes.status()).toBe(200);

    // List and verify
    const listRes = await request.get(`${backendUrl}/api/v1/notebooks`, { headers });
    const list = await listRes.json();
    const found = list.notebooks.find((n: any) => n.id === nb.id);
    expect(found?.name).toBe("crud-test-renamed");

    // Delete
    const delRes = await request.delete(`${backendUrl}/api/v1/notebooks/${nb.id}`, { headers });
    expect(delRes.status()).toBe(200);
  });
});
