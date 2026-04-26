import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  changePassword,
  confirmResetPassword,
  sendResetCode,
  verifyResetCode,
} from "../../lib/auth/client";

const fetchMock = vi.fn();

beforeEach(() => {
  process.env.NEXT_PUBLIC_API_BASE_URL = "https://api.example.test";
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  delete process.env.NEXT_PUBLIC_API_BASE_URL;
  vi.unstubAllGlobals();
});

describe("auth client reset contracts", () => {
  it("posts changePassword with the expected payload and auth header", async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ success: true, data: null, error: null }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    await expect(
      changePassword("token-123", {
        old_password: "old-pass",
        new_password: "new-pass",
      }),
    ).resolves.toEqual({ success: true, data: null, error: null });

    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/auth/change-password",
      expect.objectContaining({
        method: "POST",
        cache: "no-store",
        body: JSON.stringify({
          old_password: "old-pass",
          new_password: "new-pass",
        }),
      }),
    );

    const [, init] = fetchMock.mock.calls[0]!;
    const headers = new Headers(init.headers);

    expect(headers.get("Accept")).toBe("application/json");
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(headers.get("Authorization")).toBe("Bearer token-123");
  });

  it.each([
    [
      "sendResetCode",
      () =>
        sendResetCode({
          email: "user@example.com",
          lang: "zh-CN",
        }),
      "/api/auth/reset/send-code",
      JSON.stringify({ email: "user@example.com", lang: "zh-CN" }),
    ],
    [
      "verifyResetCode",
      () =>
        verifyResetCode({
          email: "user@example.com",
          code: "123456",
        }),
      "/api/auth/reset/verify-code",
      JSON.stringify({ email: "user@example.com", code: "123456" }),
    ],
    [
      "confirmResetPassword",
      () =>
        confirmResetPassword({
          reset_ticket: "ticket-123",
          new_password: "new-pass",
        }),
      "/api/auth/reset/confirm",
      JSON.stringify({
        reset_ticket: "ticket-123",
        new_password: "new-pass",
      }),
    ],
  ])("posts %s with the expected payload", async (_name, call, path, body) => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ success: true, data: null, error: null }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );

    await expect(call()).resolves.toEqual({ success: true, data: null, error: null });

    expect(fetchMock).toHaveBeenCalledWith(
      `https://api.example.test${path}`,
      expect.objectContaining({
        method: "POST",
        cache: "no-store",
        body,
      }),
    );

    const [, init] = fetchMock.mock.calls[0]!;
    const headers = new Headers(init.headers);

    expect(headers.get("Accept")).toBe("application/json");
    expect(headers.get("Content-Type")).toBe("application/json");
    expect(headers.get("Authorization")).toBeNull();
  });
});
