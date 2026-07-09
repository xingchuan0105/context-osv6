import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock("@/lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(() => true),
}));

import {
  activateLicense,
  formatLicenseError,
  getDeviceId,
  getLicenseStatus,
  licenseKindLabel,
  listenDeepLinkActivate,
  startTrial,
} from "@/lib/desktop/tauri-license";

describe("tauri-license", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("getDeviceId_invokesGetDeviceIdCommand", async () => {
    invokeMock.mockResolvedValueOnce("device-123");

    await expect(getDeviceId()).resolves.toBe("device-123");
    expect(invokeMock).toHaveBeenCalledWith("get_device_id", undefined);
  });

  it("activateLicense_passesSnakeCaseLicenseKey", async () => {
    invokeMock.mockResolvedValueOnce({
      license_id: "lic-1",
      kind: "pro",
      status: "active",
    });

    await activateLicense("AVRG-TEST");

    expect(invokeMock).toHaveBeenCalledWith("activate_license", {
      license_key: "AVRG-TEST",
    });
  });

  it("startTrial_returnsTrialResult", async () => {
    invokeMock.mockResolvedValueOnce({ expires_at: 1_700_000_000, days_remaining: 7 });

    const result = await startTrial();

    expect(result.days_remaining).toBe(7);
    expect(invokeMock).toHaveBeenCalledWith("start_trial", undefined);
  });

  it("getLicenseStatus_readsLicenseStatus", async () => {
    invokeMock.mockResolvedValueOnce({ kind: "trial", days_remaining: 5, dev_mode: true });

    const status = await getLicenseStatus();

    expect(status.kind).toBe("trial");
    expect(status.days_remaining).toBe(5);
  });

  it("listenDeepLinkActivate_registersDeepLinkListener", async () => {
    const handler = vi.fn();
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);

    const cleanup = await listenDeepLinkActivate(handler);
    listenMock.mock.calls[0][1]({ payload: "AVRG-KEY" });

    expect(handler).toHaveBeenCalledWith("AVRG-KEY");
    expect(typeof cleanup).toBe("function");
  });

  it("formatLicenseError_extractsStructuredMessage", () => {
    expect(formatLicenseError({ code: "invalid", message: "授权码无效" })).toBe("授权码无效");
    expect(formatLicenseError("plain error")).toBe("plain error");
  });

  it("licenseKindLabel_mapsKinds", () => {
    expect(licenseKindLabel("pro")).toBe("AVRag Desktop Pro");
    expect(licenseKindLabel("standard")).toBe("AVRag Desktop Standard");
    expect(licenseKindLabel("trial")).toBe("AVRag Desktop Trial");
  });
});
