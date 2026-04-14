import { expect, type APIRequestContext, type Page } from "@playwright/test";

export function uniqueName(prefix: string): string {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

export async function seedLocalAuth(page: Page, token: string) {
  await page.addInitScript((value) => {
    window.localStorage.setItem("token", value);
  }, token);
}

export async function gotoStable(page: Page, url: string) {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await page.goto(url, { waitUntil: "domcontentloaded" });
    await page.waitForTimeout(1500);

    const chunkErrorVisible = await page
      .getByText("Runtime ChunkLoadError")
      .isVisible()
      .catch(() => false);
    const appErrorVisible = await page
      .getByText("Application error:")
      .isVisible()
      .catch(() => false);

    if (!chunkErrorVisible && !appErrorVisible) {
      return;
    }

    await page.waitForTimeout(1000);
  }

  throw new Error(`failed to load stable page: ${url}`);
}

export async function waitForAppReady(page: Page) {
  const loading = page.getByText("加载中...");
  await loading.waitFor({ state: "hidden", timeout: 15_000 }).catch(() => {});
}

export async function createNotebookViaAPI(
  request: APIRequestContext,
  token: string,
  name: string,
  description = "playwright e2e",
): Promise<string> {
  const response = await request.post("/api/v1/notebooks", {
    headers: {
      Authorization: `Bearer ${token}`,
    },
    data: {
      name,
      description,
    },
  });
  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { notebook?: { id?: string } };
  const id = String(payload?.notebook?.id || "");
  expect(id).not.toBe("");

  for (let attempt = 0; attempt < 10; attempt += 1) {
    const ready = await request.get(`/api/v1/notebooks/${id}`, {
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    if (ready.ok()) {
      return id;
    }
    await new Promise((resolve) => setTimeout(resolve, 300));
  }

  return id;
}

export async function registerTestUser(
  request: APIRequestContext,
): Promise<{ token: string; email: string; password: string }> {
  const email = `${uniqueName("pw-user")}@local.test`;
  const password = "Playwright123!";
  // Directly hit the backend if possible, or ensure we follow rewrites.
  // In many CI/local dev envs, hitting 8080 directly is more reliable for APIRequestContext
  const response = await request.post("http://localhost:8080/api/auth/register", {
    data: {
      email,
      password,
      full_name: "Playwright User",
    },
  });
  expect(response.ok()).toBeTruthy();
  const payload = (await response.json()) as { data?: { token?: string } };
  const token = String(payload?.data?.token || "").trim();
  expect(token).not.toBe("");
  return { token, email, password };
}

export async function deleteNotebookViaAPI(
  request: APIRequestContext,
  token: string,
  notebookId: string,
) {
  if (!notebookId) {
    return;
  }
  await request.delete(`/api/v1/notebooks/${notebookId}`, {
    headers: {
      Authorization: `Bearer ${token}`,
    },
  });
}
