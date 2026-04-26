import type { AuthUser } from "./client";

const AUTH_SESSION_COOKIE_NAME = "avrag.auth.session";
const AUTH_SESSION_COOKIE_VALUE = "1";
const AUTH_PERSISTED_COOKIE_NAME = "avrag.auth.persisted";
const AUTH_SESSION_COOKIE_MAX_AGE = 60 * 60 * 24 * 365;

type PersistedAuthCookie = {
  token: string;
  user: AuthUser;
};

function buildCookieString(maxAge: number) {
  return `${AUTH_SESSION_COOKIE_NAME}=${AUTH_SESSION_COOKIE_VALUE}; Path=/; SameSite=Lax; Max-Age=${maxAge}`;
}

function buildPersistedCookieString(value: string, maxAge: number) {
  return `${AUTH_PERSISTED_COOKIE_NAME}=${value}; Path=/; SameSite=Lax; Max-Age=${maxAge}`;
}

export function setAuthSessionHint() {
  if (typeof document === "undefined") {
    return;
  }

  document.cookie = buildCookieString(AUTH_SESSION_COOKIE_MAX_AGE);
}

export function clearAuthSessionHint() {
  if (typeof document === "undefined") {
    return;
  }

  document.cookie = buildCookieString(0);
}

export function writePersistedAuthCookie(token: string, user: AuthUser) {
  if (typeof document === "undefined") {
    return;
  }

  const payload = encodeURIComponent(JSON.stringify({ token, user } satisfies PersistedAuthCookie));
  document.cookie = buildPersistedCookieString(payload, AUTH_SESSION_COOKIE_MAX_AGE);
}

export function readPersistedAuthCookie(): PersistedAuthCookie | null {
  if (typeof document === "undefined") {
    return null;
  }

  const cookieEntry = document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${AUTH_PERSISTED_COOKIE_NAME}=`));

  if (!cookieEntry) {
    return null;
  }

  const rawValue = cookieEntry.slice(AUTH_PERSISTED_COOKIE_NAME.length + 1);

  try {
    return JSON.parse(decodeURIComponent(rawValue)) as PersistedAuthCookie;
  } catch {
    return null;
  }
}

export function clearPersistedAuthCookie() {
  if (typeof document === "undefined") {
    return;
  }

  document.cookie = buildPersistedCookieString("", 0);
}

export { AUTH_PERSISTED_COOKIE_NAME, AUTH_SESSION_COOKIE_NAME };
