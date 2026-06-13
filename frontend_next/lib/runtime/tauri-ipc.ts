/**
 * Tauri IPC 传输层
 *
 * 通过 Tauri invoke 调用本地 Rust 核心，替代 HTTP/SSE
 * 使用 @tauri-apps/api 官方 API，不直接访问 internals
 */

import { ApiError } from "../auth/client";
import type { ChatRequest, ChatEvent } from "../contracts";
import { parseIpcChatEvent } from "../workspace/stream";

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
 * 动态加载 Tauri API（仅在 Tauri 环境中）
 */
async function getTauriInvoke() {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke;
}

async function getTauriListen() {
  const { listen } = await import("@tauri-apps/api/event");
  return listen;
}

type StructuredIpcError = {
  status: number;
  code?: string | null;
  message: string;
};

function isStructuredIpcError(value: unknown): value is StructuredIpcError {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Record<string, unknown>;
  return typeof candidate.status === "number" && typeof candidate.message === "string";
}

function mapInvokeError(error: unknown): Error {
  if (isStructuredIpcError(error)) {
    return new ApiError(
      error.status,
      typeof error.code === "string" ? error.code : null,
      error.message,
    );
  }

  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      if (isStructuredIpcError(parsed)) {
        return new ApiError(
          parsed.status,
          typeof parsed.code === "string" ? parsed.code : null,
          parsed.message,
        );
      }
    } catch {
      return new Error(error);
    }

    return new Error(error);
  }

  return error instanceof Error ? error : new Error(String(error));
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new DOMException("The operation was aborted.", "AbortError");
  }
}

/**
 * 初始化本地后端
 */
export async function initLocalBackend(): Promise<string> {
  const invoke = await getTauriInvoke();
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
  const invoke = await getTauriInvoke();
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
  const invoke = await getTauriInvoke();
  return invoke("list_local_documents");
}

/**
 * 获取缓存值
 */
export async function getCacheValue(key: string): Promise<string | null> {
  const invoke = await getTauriInvoke();
  return invoke("get_cache_value", { key });
}

/**
 * 设置缓存值
 */
export async function setCacheValue(key: string, value: string, ttlSecs: number): Promise<void> {
  const invoke = await getTauriInvoke();
  return invoke("set_cache_value", { key, value, ttl_secs: ttlSecs });
}

/**
 * 流式聊天（通过 Tauri IPC）
 *
 * 通过 Tauri command 发起聊天，通过事件接收流式响应
 */
export async function streamChatViaIPC(
  token: string,
  request: ChatRequest,
  onEvent: (event: ChatEvent) => void | Promise<void>,
  options?: { signal?: AbortSignal },
): Promise<void> {
  const invoke = await getTauriInvoke();
  const listen = await getTauriListen();

  const requestId = crypto.randomUUID();
  const signal = options?.signal;
  let unlisten: (() => void) | null = null;
  let abortHandler: (() => void) | null = null;

  const cleanupListener = () => {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  };

  throwIfAborted(signal);

  try {
    unlisten = await listen(`chat://${requestId}`, (e) => {
      const event = parseIpcChatEvent(e.payload);

      if (event) {
        void onEvent(event);
      }
    });

    abortHandler = () => {
      cleanupListener();
      void invoke("chat_cancel", { request_id: requestId }).catch(() => {});
    };

    signal?.addEventListener("abort", abortHandler, { once: true });

    await invoke("chat_stream", {
      token,
      request: {
        ...request,
        request_id: requestId,
        stream: true,
      },
    });

    throwIfAborted(signal);
  } finally {
    if (abortHandler && signal) {
      signal.removeEventListener("abort", abortHandler);
    }
    cleanupListener();
  }
}

/**
 * REST 请求（通过 Tauri IPC）
 */
export async function requestViaIPC<T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> {
  const invoke = await getTauriInvoke();

  let body: unknown = null;

  if (init?.body != null) {
    if (typeof init.body !== "string") {
      throw new TypeError("requestViaIPC only supports JSON string bodies");
    }

    body = JSON.parse(init.body);
  }

  try {
    return await invoke<T>("api_call", {
      method: init?.method || "GET",
      path,
      body,
      token,
    });
  } catch (error) {
    throw mapInvokeError(error);
  }
}
