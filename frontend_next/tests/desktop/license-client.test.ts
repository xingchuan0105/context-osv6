import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createLicenseCheckout,
  fetchLicenseMachines,
  fetchMyLicenses,
  formatHeartbeatLabel,
  licenseKindDisplay,
  licenseStatusLabel,
} from "@/lib/desktop/license-client";

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

describe("license-client", () => {
  it("fetchMyLicenses_callsLicensesMeEndpoint", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: {
            licenses: [
              {
                id: "lic-1",
                key: "AVRG-PRO1-XXXX",
                status: "active",
                kind: "pro",
                max_machines: 3,
                machines_count: 1,
              },
            ],
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await fetchMyLicenses("token-1");

    expect(result.licenses).toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/licenses/me",
      expect.objectContaining({ method: "GET" }),
    );

    const requestInit = fetchMock.mock.calls[0][1] as RequestInit;
    const headers = new Headers(requestInit.headers);
    expect(headers.get("Authorization")).toBe("Bearer token-1");
  });

  it("fetchLicenseMachines_callsMachinesEndpoint", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: {
            machines: [{ id: "m1", name: "Desktop-1", last_heartbeat_at: "2026-07-08T10:00:00Z" }],
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await fetchLicenseMachines("token-1", "lic-1");

    expect(result.machines[0].name).toBe("Desktop-1");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/licenses/lic-1/machines",
      expect.objectContaining({ method: "GET" }),
    );
  });

  it("createLicenseCheckout_postsCheckoutPayload", async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          data: {
            checkout_url: "https://checkout.example.test/session-1",
            session_id: "session-1",
            plan_id: "pro",
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );

    const result = await createLicenseCheckout("token-1", {
      plan_id: "pro",
      device_id: "device-1",
    });

    expect(result.checkout_url).toContain("checkout.example.test");
    expect(fetchMock).toHaveBeenCalledWith(
      "https://api.example.test/api/v1/licenses/checkout",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          plan_id: "pro",
          device_id: "device-1",
        }),
      }),
    );
  });

  it("licenseStatusLabel_mapsKnownStatuses", () => {
    expect(licenseStatusLabel("active")).toBe("已激活");
    expect(licenseStatusLabel("trial")).toBe("试用中");
    expect(licenseKindDisplay("pro")).toBe("AVRag Desktop Pro");
  });

  it("formatHeartbeatLabel_formatsRecentHeartbeat", () => {
    const recent = new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString();
    expect(formatHeartbeatLabel(recent)).toBe("2 小时前");
  });
});
