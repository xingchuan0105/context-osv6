import type { APIRequestContext } from '@playwright/test';

export const LIVE_API_BASE = (process.env.LIVE_API_BASE || 'http://127.0.0.1:8080').replace(
  /\/$/,
  '',
);

const TERMS_VERSION = '2026-06-13';
const PRIVACY_VERSION = '2026-06-13';

type AuthEnvelope = {
  success?: boolean;
  data?: { token?: string } | null;
  error?: string | null;
};

export async function obtainLiveJwt(
  request: APIRequestContext,
  apiBase = LIVE_API_BASE,
): Promise<string> {
  const existing = process.env.LIVE_JWT?.trim();
  if (existing) {
    return existing;
  }

  const email = process.env.E2E_TEST_USER_EMAIL || 'e2e-test@example.com';
  const password = process.env.E2E_TEST_USER_PASSWORD || 'E2eTest123!';
  const login = await request.post(`${apiBase}/api/auth/login`, {
    data: { email, password },
  });
  const loginBody = (await login.json().catch(() => ({}))) as AuthEnvelope;
  if (login.ok() && loginBody.data?.token) {
    return loginBody.data.token;
  }

  if (loginBody.error !== 'account_not_registered') {
    throw new Error(`live login failed with HTTP ${login.status()}`);
  }

  const register = await request.post(`${apiBase}/api/auth/register`, {
    data: {
      email,
      password,
      full_name: 'E2E Test User',
      terms_version: TERMS_VERSION,
      privacy_version: PRIVACY_VERSION,
    },
  });
  const registerBody = (await register.json().catch(() => ({}))) as AuthEnvelope;
  if (register.ok() && registerBody.data?.token) {
    return registerBody.data.token;
  }

  const retry = await request.post(`${apiBase}/api/auth/login`, {
    data: { email, password },
  });
  const retryBody = (await retry.json().catch(() => ({}))) as AuthEnvelope;
  if (retry.ok() && retryBody.data?.token) {
    return retryBody.data.token;
  }
  throw new Error(`live register/login failed with HTTP ${register.status()}/${retry.status()}`);
}
