import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

describe("desktop document upload", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({ ok: true, status: 200 });
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: { __TAURI_INTERNALS__: {} },
    });
    vi.stubGlobal("fetch", vi.fn());
  });

  afterEach(() => {
    vi.resetModules();
    vi.unstubAllGlobals();
  });

  it("puts file bytes through Tauri IPC instead of window.fetch", async () => {
    const { uploadWorkspaceDocumentFile } = await import("../../lib/workspace/client");
    const file = new File(["hello"], "notes.txt", { type: "text/plain" });

    await uploadWorkspaceDocumentFile(
      "http://127.0.0.1:18080/uploads/doc-1?expires=1&signature=abc",
      file,
      "text/plain",
    );

    expect(fetch).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("upload_bytes", {
      url: "http://127.0.0.1:18080/uploads/doc-1?expires=1&signature=abc",
      contentType: "text/plain",
      bodyBase64: btoa("hello"),
    });
  });
});
