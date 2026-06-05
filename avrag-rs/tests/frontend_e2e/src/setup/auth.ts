import { Page, APIRequestContext } from "@playwright/test";
import { getBackendBaseUrl } from "./backendUrl";

export interface TestAuth {
  token: string;
  user: { id: string; email: string; full_name: string };
}

/**
 * Register a fresh test user against the backend and return the auth payload.
 * Each test gets a unique user so parallel runs don't collide.
 * Retries up to 3 times on transient 5xx errors.
 */
export async function registerTestUser(request: APIRequestContext): Promise<TestAuth> {
  const backendUrl = getBackendBaseUrl();
  const random = Math.random().toString(36).slice(2, 10);
  const email = `e2e-${random}@example.com`;
  const password = "TestPassword123!";

  for (let attempt = 1; attempt <= 3; attempt++) {
    const res = await request.post(`${backendUrl}/api/auth/register`, {
      data: {
        email,
        password,
        full_name: `E2E User ${random}`,
      },
    });

    if (res.ok()) {
      const json = (await res.json()) as any;
      return json.data as TestAuth;
    }

    const body = await res.text();
    if (attempt === 3 || res.status() < 500) {
      throw new Error(`Register failed: ${res.status()} ${body}`);
    }
    // Wait before retry on 5xx
    await new Promise((r) => setTimeout(r, 500 * attempt));
  }

  throw new Error("Register failed after 3 attempts");
}

/**
 * Inject an auth token into the page via addInitScript so the React AuthProvider
 * picks it up on load before any hydration occurs.
 */
export async function injectAuth(page: Page, auth: TestAuth): Promise<void> {
  await page.addInitScript(({ token, user }) => {
    localStorage.setItem("avrag.auth.v1", JSON.stringify({ token, user }));
  }, auth);
}

/**
 * Return headers for API calls using the registered user's Bearer token.
 */
export function authHeaders(auth: TestAuth): Record<string, string> {
  return {
    "Authorization": `Bearer ${auth.token}`,
    "Content-Type": "application/json",
  };
}
