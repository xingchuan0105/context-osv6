/**
 * 运行时传输适配层
 *
 * 根据运行环境（Tauri 桌面端 vs Web 浏览端）选择不同的传输实现。
 * 每次调用时动态判断环境，避免模块加载时的副作用。
 */

import type { ChatRequest, WorkspaceChatStreamEvent } from "../workspace/stream";

export { isTauri } from "./tauri-ipc";

/**
 * 流式聊天传输：根据运行环境选择实现
 */
export async function streamChat(
  token: string,
  request: ChatRequest,
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
  options?: { signal?: AbortSignal },
): Promise<void> {
  const { isTauri } = await import("./tauri-ipc");
  if (isTauri()) {
    const { streamChatViaIPC } = await import("./tauri-ipc");
    return streamChatViaIPC(token, request, onEvent, options);
  }
  const { streamWorkspaceChat } = await import("../workspace/stream");
  return streamWorkspaceChat(token, request, onEvent, options);
}

/**
 * REST 请求传输：根据运行环境选择实现
 */
export async function restRequest<T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> {
  const { isTauri } = await import("./tauri-ipc");
  if (isTauri()) {
    const { requestViaIPC } = await import("./tauri-ipc");
    return requestViaIPC<T>(path, init, token);
  }
  const { request } = await import("../auth/client");
  return request<T>(path, init, token);
}

/**
 * 初始化本地后端（仅桌面端）
 */
export async function initLocalBackend(): Promise<void> {
  const { isTauri } = await import("./tauri-ipc");
  if (!isTauri()) return;
  const { initLocalBackend: init } = await import("./tauri-ipc");
  await init();
}

/**
 * 获取后端状态（仅桌面端）
 */
export async function getBackendStatus(): Promise<{
  initialized: boolean;
  type: string;
} | null> {
  const { isTauri } = await import("./tauri-ipc");
  if (!isTauri()) return null;
  const { getBackendStatus: getStatus } = await import("./tauri-ipc");
  return getStatus();
}
