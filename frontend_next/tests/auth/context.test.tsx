import { act, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createElement } from "react";

const mocks = vi.hoisted(() => ({
  meMock: vi.fn(),
  logoutMock: vi.fn(),
  authRuntimeCapabilitiesMock: vi.fn(),
}));

vi.mock("../../lib/auth/client", () => ({
  authRuntimeCapabilities: mocks.authRuntimeCapabilitiesMock,
  logout: mocks.logoutMock,
  me: mocks.meMock,
}));

import { AuthProvider, useAuth } from "../../lib/auth/context";
import type { AuthPayload } from "../../lib/auth/client";

type AuthApi = {
  initialized: boolean;
  isAuthenticated: boolean;
  completeAuth: (payload: AuthPayload) => void;
  clearAuth: () => void;
};

function AuthProbe({
  onReady,
}: {
  onReady: (auth: AuthApi) => void;
}) {
  const auth = useAuth();

  onReady(auth);

  return null;
}

beforeEach(() => {
  window.localStorage.clear();
  document.cookie = "avrag.auth.session=; Path=/; Max-Age=0";
  document.cookie = "avrag.auth.persisted=; Path=/; Max-Age=0";
  mocks.meMock.mockReset();
  mocks.logoutMock.mockReset();
  mocks.authRuntimeCapabilitiesMock.mockReset();
  mocks.authRuntimeCapabilitiesMock.mockResolvedValue({ password_reset_enabled: false });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("AuthProvider session hint cookie", () => {
  it("sets the auth session hint when auth completes", async () => {
    let authApi: AuthApi | null = null;

    render(
      createElement(AuthProvider, {
        children: createElement(AuthProbe, {
          onReady: (auth) => {
            authApi = auth;
          },
        }),
      }),
    );

    await waitFor(() => {
      expect(authApi).not.toBeNull();
    });

    if (!authApi) {
      throw new Error("auth api not ready");
    }

    const readyAuthApi = authApi as AuthApi;

    readyAuthApi.completeAuth({
      token: "token-1",
      user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
      reset_ticket: null,
    });

    expect(window.localStorage.getItem("avrag.auth.v1")).toBe(
      JSON.stringify({
        token: "token-1",
        user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
      }),
    );
    expect(document.cookie).toContain("avrag.auth.session=");
    expect(document.cookie).toContain("avrag.auth.persisted=");

    readyAuthApi.clearAuth();

    expect(window.localStorage.getItem("avrag.auth.v1")).toBeNull();
    expect(document.cookie).not.toContain("avrag.auth.session=");
    expect(document.cookie).not.toContain("avrag.auth.persisted=");
  });

  it("clears the auth session hint when bootstrap invalidates stale local auth", async () => {
    window.localStorage.setItem(
      "avrag.auth.v1",
      JSON.stringify({
        token: "stale-token",
        user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
      }),
    );
    mocks.meMock.mockResolvedValue({
      success: false,
      data: null,
      error: "invalid",
    });

    render(
      createElement(AuthProvider, {
        children: createElement("div"),
      }),
    );

    await waitFor(() => {
      expect(window.localStorage.getItem("avrag.auth.v1")).toBeNull();
    });

    expect(document.cookie).not.toContain("avrag.auth.session=");
    expect(document.cookie).not.toContain("avrag.auth.persisted=");
  });

  it("finishes bootstrap when persisted auth validation hangs", async () => {
    vi.useFakeTimers();
    window.localStorage.setItem(
      "avrag.auth.v1",
      JSON.stringify({
        token: "stale-token",
        user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
      }),
    );
    mocks.meMock.mockImplementation(() => new Promise(() => {}));

    let authApi: AuthApi | null = null;

    try {
      render(
        createElement(AuthProvider, {
          children: createElement(AuthProbe, {
            onReady: (auth) => {
              authApi = auth;
            },
          }),
        }),
      );

      expect(authApi).not.toBeNull();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000);
      });

      if (!authApi) {
        throw new Error("auth api not ready");
      }

      const readyAuthApi = authApi as AuthApi;

      expect(readyAuthApi.initialized).toBe(true);
      expect(readyAuthApi.isAuthenticated).toBe(false);
      expect(window.localStorage.getItem("avrag.auth.v1")).toBeNull();
      expect(document.cookie).not.toContain("avrag.auth.session=");
      expect(document.cookie).not.toContain("avrag.auth.persisted=");
    } finally {
      vi.useRealTimers();
    }
  });

  it("bootstraps from cookie-backed auth when localStorage is unavailable", async () => {
    const getItemSpy = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new Error("storage unavailable");
      });

    document.cookie = `avrag.auth.persisted=${encodeURIComponent(
      JSON.stringify({
        token: "cookie-token",
        user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
      }),
    )}; Path=/`;

    mocks.meMock.mockResolvedValue({
      success: true,
      data: {
        token: "",
        reset_ticket: null,
        user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
      },
      error: null,
    });

    let authApi: AuthApi | null = null;

    render(
      createElement(AuthProvider, {
        children: createElement(AuthProbe, {
          onReady: (auth) => {
            authApi = auth;
          },
        }),
      }),
    );

    await waitFor(() => {
      expect(authApi?.initialized).toBe(true);
      expect(authApi?.isAuthenticated).toBe(true);
    });

    getItemSpy.mockRestore();
  });
});
