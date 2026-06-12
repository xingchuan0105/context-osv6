/**
 * Tauri IPC 传输层
 *
 * 阶段 2：通过 Tauri invoke 调用本地 Rust 核心，替代 HTTP/SSE
 */

import type { ChatRequest, WorkspaceChatStreamEvent } from "../workspace/stream";
import type { ApiError } from "../auth/client";

/**
 * 检测是否在 Tauri 环境中运行
 */
export function isTauri(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  return "__TAURI_INTERNALS__" in window;
}

/**
 * 调用 Tauri command
 */
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error("Not in Tauri environment");
  }

  // @ts-expect-error Tauri API is injected at runtime
  return window.__TAURI_INTERNALS__.invoke(command, args);
}

/**
 * 监听 Tauri 事件
 */
async function listen(event: string, handler: (event: { payload: unknown }) => void): Promise<() => void> {
  if (!isTauri()) {
    throw new Error("Not in Tauri environment");
  }

  // @ts-expect-error Tauri API is injected at runtime
  return window.__TAURI_INTERNALS__.listen(event, handler);
}

/**
 * 初始化本地后端
 */
export async function initLocalBackend(): Promise<string> {
  return invoke<string>("init_local_backend");
}

/**
 * 获取后端状态
 */
export async function getBackendStatus(): Promise<{
  initialized: boolean;
  type: string;
  storage: { type: string; initialized: boolean };
  cache: { type: string; initialized: boolean };
}> {
  return invoke("get_backend_status");
}

/**
 * 列出本地文档
 */
export async function listLocalDocuments(): Promise<Array<{
  id: string;
  name: string;
  status: string;
  created_at: string;
}>> {
  return invoke("list_local_documents");
}

/**
 * 获取缓存值
 */
export async function getCacheValue(key: string): Promise<string | null> {
  return invoke("get_cache_value", { key });
}

/**
 * 设置缓存值
 */
export async function setCacheValue(key: string, value: string, ttlSecs: number): Promise<void> {
  return invoke("set_cache_value", { key, value, ttl_secs: ttlSecs });
}

/**
 * 流式聊天（通过 Tauri IPC）
 *
 * 阶段 2 实现：通过 Tauri command 发起聊天，通过事件接收流式响应
 */
export async function streamChatViaIPC(
  token: string,
  request: ChatRequest,
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
  options?: { signal?: AbortSignal },
): Promise<void> {
  const requestId = crypto.randomUUID();

  // 监听聊天事件
  const unlisten = await listen(`chat://${requestId}`, (e) => {
    const event = e.payload as WorkspaceChatStreamEvent;
    onEvent(event);
  });

  try {
    // 发起聊天请求
    await invoke("chat_stream", {
      token,
      request: {
        ...request,
        request_id: requestId,
        stream: true,
      },
    });
  } finally {
    // 清理监听器
    unlisten();
  }
}

/**
 * REST 请求（通过 Tauri IPC）
 *
 * 阶段 2 实现：通过 Tauri command 发起 REST 请求
 */
export async function requestViaIPC<T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> {
  return invoke("api_call", {
    method: init?.method || "GET",
    path,
    body: init?.body ? JSON.parse(init.body as string) : null,
    token,
  });
}
