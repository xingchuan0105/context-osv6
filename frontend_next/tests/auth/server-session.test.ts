import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  AUTH_PERSISTED_COOKIE_NAME,
  AUTH_SESSION_COOKIE_NAME,
  writePersistedAuthCookie,
  setAuthSessionHint,
} from "../../lib/auth/server-session";
import type { AuthUser } from "../../lib/auth/client";

const user: AuthUser = {
  id: "u1",
  email: "a@b.c",
  full_name: "A",
};

describe("auth cookies", () => {
  beforeEach(() => {
    document.cookie = `${AUTH_SESSION_COOKIE_NAME}=; Path=/; Max-Age=0`;
    document.cookie = `${AUTH_PERSISTED_COOKIE_NAME}=; Path=/; Max-Age=0`;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("writes SameSite=Lax on persisted and session cookies", () => {
    const setter = vi.spyOn(document, "cookie", "set");
    setAuthSessionHint();
    writePersistedAuthCookie("token", user);

    const written = setter.mock.calls.map((call) => String(call[0]));
    expect(written.some((c) => c.startsWith(`${AUTH_SESSION_COOKIE_NAME}=`) && c.includes("SameSite=Lax"))).toBe(
      true,
    );
    expect(
      written.some((c) => c.startsWith(`${AUTH_PERSISTED_COOKIE_NAME}=`) && c.includes("SameSite=Lax")),
    ).toBe(true);
  });

  it("adds Secure only on https", () => {
    const setter = vi.spyOn(document, "cookie", "set");
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { protocol: "https:" },
    });

    writePersistedAuthCookie("token", user);

    const written = setter.mock.calls.map((call) => String(call[0]));
    expect(written.some((c) => c.includes("Secure") && c.includes("SameSite=Lax"))).toBe(true);
  });
});
