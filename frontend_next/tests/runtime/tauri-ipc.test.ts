import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const listenMock = vi.fn();
const unlistenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

describe("tauri-ipc runtime", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    unlistenMock.mockReset();
    listenMock.mockResolvedValue(unlistenMock);
    Object.defineProperty(globalThis, "window", {
      configurable: true,
      value: { __TAURI_INTERNALS__: {} },
    });
  });

  afterEach(() => {
    vi.resetModules();
    vi.unstubAllGlobals();
  });

  it("requestViaIPC serializes JSON string bodies for api_call", async () => {
    invokeMock.mockResolvedValue({ ok: true });

    const { requestViaIPC } = await import("../../lib/runtime/tauri-ipc");

    await requestViaIPC("/api/v1/workspaces", {
      method: "POST",
      body: JSON.stringify({ name: "demo" }),
    });

    expect(invokeMock).toHaveBeenCalledWith("api_call", {
      method: "POST",
      path: "/api/v1/workspaces",
      body: { name: "demo" },
      token: undefined,
    });
  });

  it("requestViaIPC sends null body when init.body is omitted", async () => {
    invokeMock.mockResolvedValue({ ok: true });

    const { requestViaIPC } = await import("../../lib/runtime/tauri-ipc");

    await requestViaIPC("/api/v1/health");

    expect(invokeMock).toHaveBeenCalledWith("api_call", {
      method: "GET",
      path: "/api/v1/health",
      body: null,
      token: undefined,
    });
  });

  it("requestViaIPC throws TypeError for non-string bodies", async () => {
    const { requestViaIPC } = await import("../../lib/runtime/tauri-ipc");

    await expect(
      requestViaIPC("/api/v1/upload", {
        method: "POST",
        body: new FormData(),
      }),
    ).rejects.toThrow(TypeError);
  });

  it("requestViaIPC maps structured invoke errors to ApiError", async () => {
    invokeMock.mockRejectedValue({
      status: 501,
      code: "not_implemented",
      message: "desktop placeholder",
    });

    const { requestViaIPC } = await import("../../lib/runtime/tauri-ipc");

    await expect(requestViaIPC("/api/v1/settings")).rejects.toMatchObject({
      name: "ApiError",
      status: 501,
      code: "not_implemented",
      message: "desktop placeholder",
    });
  });

  it("streamChatViaIPC forwards parsed events in emission order", async () => {
    let eventHandler: ((event: { payload: unknown }) => void) | null = null;

    listenMock.mockImplementation(async (_channel: string, handler: (event: { payload: unknown }) => void) => {
      eventHandler = handler;
      return unlistenMock;
    });

    invokeMock.mockImplementation(async () => {
      eventHandler?.({
        payload: {
          event: "start",
          request_id: "req-ipc",
          session_id: "sess-ipc",
        },
      });
      eventHandler?.({
        payload: {
          event: "token",
          request_id: "req-ipc",
          message_id: 1,
          content: "Hi",
        },
      });
      eventHandler?.({
        payload: {
          event: "done",
          request_id: "req-ipc",
          session_id: "sess-ipc",
          message_id: 1,
          payload: { answer: "Hi" },
        },
      });
    });

    const { streamChatViaIPC } = await import("../../lib/runtime/tauri-ipc");
    const events: Array<{ event: string }> = [];

    await streamChatViaIPC(
      "token-1",
      {
        query: "hello",
        workspace_id: "ws-1",
        session_id: undefined,
        agent_type: "chat",
        doc_scope: [],
        messages: [],
        stream: true,
      },
      (event) => {
        events.push(event);
      },
    );

    expect(events.map((event) => event.event)).toEqual(["start", "token", "done"]);
    expect(unlistenMock).toHaveBeenCalledTimes(1);
  });

  it("streamChatViaIPC invokes chat_cancel and unlistens when aborted", async () => {
    let eventHandler: ((event: { payload: unknown }) => void) | null = null;
    let capturedRequestId: string | null = null;

    listenMock.mockImplementation(async (_channel: string, handler: (event: { payload: unknown }) => void) => {
      eventHandler = handler;
      return unlistenMock;
    });

    invokeMock.mockImplementation(async (command: string, args?: { request?: { request_id?: string } }) => {
      if (command === "chat_stream") {
        capturedRequestId = args?.request?.request_id ?? null;
        eventHandler?.({
          payload: {
            event: "token",
            request_id: capturedRequestId,
            message_id: 1,
            content: "partial",
          },
        });

        await new Promise<void>((resolve) => {
          setTimeout(resolve, 20);
        });
      }
    });

    const { streamChatViaIPC } = await import("../../lib/runtime/tauri-ipc");
    const controller = new AbortController();

    const streamPromise = streamChatViaIPC(
      "token-1",
      {
        query: "hello",
        workspace_id: "ws-1",
        session_id: undefined,
        agent_type: "chat",
        doc_scope: [],
        messages: [],
        stream: true,
      },
      () => {},
      { signal: controller.signal },
    );

    setTimeout(() => controller.abort(), 5);

    await expect(streamPromise).rejects.toMatchObject({ name: "AbortError" });

    expect(capturedRequestId).toBeTruthy();
    expect(invokeMock).toHaveBeenCalledWith("chat_cancel", { request_id: capturedRequestId });
    expect(unlistenMock).toHaveBeenCalled();
  });
});
