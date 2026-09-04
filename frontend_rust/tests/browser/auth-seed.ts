import type { Page } from "@playwright/test";

export const AUTH_STORAGE_KEY = "avrag.auth.v1";

export type SeedAuthUser = {
  id: string;
  email: string;
  full_name: string;
};

export function nextAuthPayload(token: string, user?: SeedAuthUser) {
  return {
    token,
    user: user ?? {
      id: "fixture-user",
      email: "poc@example.com",
      full_name: "PoC",
    },
  };
}

/** 写入与 Next `avrag.auth.v1` / session hint / persisted cookie 相同的浏览器态。 */
export async function seedNextAuth(
  page: Page,
  token: string,
  user?: SeedAuthUser,
) {
  const payload = nextAuthPayload(token, user);
  await page.addInitScript((payload) => {
    window.localStorage.setItem("avrag.auth.v1", JSON.stringify(payload));
    const encoded = encodeURIComponent(JSON.stringify(payload));
    document.cookie = "avrag.auth.session=1; Path=/; SameSite=Lax; Max-Age=31536000";
    document.cookie = `avrag.auth.persisted=${encoded}; Path=/; SameSite=Lax; Max-Age=31536000`;
  }, payload);
}
