import { test, expect } from "../../fixtures/run-context";

test.describe("Notebook CRUD", () => {
  test("create rename and delete notebook", async ({ page, runId }) => {
    // Create
    const createRes = await page.request.post("/api/v1/notebooks", {
      data: { name: `crud-test ${runId}`, description: "" },
    });
    expect(createRes.status()).toBe(201);
    const nb = (await createRes.json()).notebook;

    // Rename
    const renameRes = await page.request.patch(`/api/v1/notebooks/${nb.id}`, {
      data: { name: `crud-test-renamed ${runId}` },
    });
    expect(renameRes.status()).toBe(200);

    // List and verify
    const listRes = await page.request.get("/api/v1/notebooks");
    const list = await listRes.json();
    const found = list.notebooks.find((n: any) => n.id === nb.id);
    expect(found?.name).toBe(`crud-test-renamed ${runId}`);

    // Delete
    const delRes = await page.request.delete(`/api/v1/notebooks/${nb.id}`);
    expect(delRes.status()).toBe(200);
  });
});
