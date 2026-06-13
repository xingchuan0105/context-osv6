import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const streamChatViaIPCMock = vi.fn();
const streamWorkspaceChatMock = vi.fn();
const requestViaIPCMock = vi.fn();
const authRequestMock = vi.fn();

vi.mock("../../lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(),
  streamChatViaIPC: (...args: unknown[]) => streamChatViaIPCMock(...args),
  requestViaIPC: (...args: unknown[]) => requestViaIPCMock(...args),
}));

vi.mock("../../lib/workspace/stream", () => ({
  streamWorkspaceChat: (...args: unknown[]) => streamWorkspaceChatMock(...args),
}));

vi.mock("../../lib/auth/client", () => ({
  request: (...args: unknown[]) => authRequestMock(...args),
}));

describe("transport runtime branches", () => {
  beforeEach(() => {
    streamChatViaIPCMock.mockReset();
    streamWorkspaceChatMock.mockReset();
    requestViaIPCMock.mockReset();
    authRequestMock.mockReset();
    vi.resetModules();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("streamChat uses IPC implementation in Tauri environment", async () => {
    const { isTauri } = await import("../../lib/runtime/tauri-ipc");
    vi.mocked(isTauri).mockReturnValue(true);

    const { streamChat } = await import("../../lib/runtime/transport");
    const onEvent = vi.fn();

    await streamChat("token", { query: "hello" } as never, onEvent, { signal: new AbortController().signal });

    expect(streamChatViaIPCMock).toHaveBeenCalledTimes(1);
    expect(streamWorkspaceChatMock).not.toHaveBeenCalled();
  });

  it("streamChat uses SSE implementation in web environment", async () => {
    const { isTauri } = await import("../../lib/runtime/tauri-ipc");
    vi.mocked(isTauri).mockReturnValue(false);

    const { streamChat } = await import("../../lib/runtime/transport");
    const onEvent = vi.fn();

    await streamChat("token", { query: "hello" } as never, onEvent);

    expect(streamWorkspaceChatMock).toHaveBeenCalledTimes(1);
    expect(streamChatViaIPCMock).not.toHaveBeenCalled();
  });

  it("restRequest uses IPC implementation in Tauri environment", async () => {
    const { isTauri } = await import("../../lib/runtime/tauri-ipc");
    vi.mocked(isTauri).mockReturnValue(true);

    const { restRequest } = await import("../../lib/runtime/transport");

    await restRequest("/api/v1/notebooks");

    expect(requestViaIPCMock).toHaveBeenCalledTimes(1);
    expect(authRequestMock).not.toHaveBeenCalled();
  });

  it("restRequest uses HTTP client in web environment", async () => {
    const { isTauri } = await import("../../lib/runtime/tauri-ipc");
    vi.mocked(isTauri).mockReturnValue(false);

    const { restRequest } = await import("../../lib/runtime/transport");

    await restRequest("/api/v1/notebooks");

    expect(authRequestMock).toHaveBeenCalledTimes(1);
    expect(requestViaIPCMock).not.toHaveBeenCalled();
  });
});
