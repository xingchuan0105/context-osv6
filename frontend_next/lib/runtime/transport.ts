/**
 * 运行时传输适配层
 *
 * 根据运行环境（Tauri 桌面端 vs Web 浏览端）选择不同的传输实现：
 * - Web 端：保持现有 SSE fetch 方式
 * - 桌面端：通过 Tauri IPC 调用本地 Rust 核心
 *
 * 这是设计文档 §4.2 的关键接缝：UI 组件、reducer、类型定义全部零改动，只在传输边界分叉。
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

  // Tauri 2 通过 __TAURI_INTERNALS__ 注入
  return "__TAURI_INTERNALS__" in window;
}

/**
 * 流式聊天传输接口
 *
 * Web 端和桌面端使用相同的函数签名，UI 层无需区分环境
 */
export type StreamChatTransport = (
  token: string,
  request: ChatRequest,
  onEvent: (event: WorkspaceChatStreamEvent) => void | Promise<void>,
  options?: { signal?: AbortSignal },
) => Promise<void>;

/**
 * REST 请求传输接口
 */
export type RestRequestTransport = <T>(
  path: string,
  init?: RequestInit,
  token?: string,
) => Promise<T>;

/**
 * 桌面端流式聊天实现（通过 Tauri IPC）
 *
 * 阶段 2：使用 Tauri IPC 替代 HTTP
 */
const tauriStreamChat: StreamChatTransport = async (token, request, onEvent, options) => {
  const { streamChatViaIPC } = await import("./tauri-ipc");
  return streamChatViaIPC(token, request, onEvent, options);
};

/**
 * 桌面端 REST 请求实现（通过 Tauri IPC）
 *
 * 阶段 2：使用 Tauri IPC 替代 HTTP
 */
const tauriRestRequest: RestRequestTransport = async <T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> => {
  const { requestViaIPC } = await import("./tauri-ipc");
  return requestViaIPC<T>(path, init, token);
};

/**
 * Web 端流式聊天实现（现有 SSE fetch）
 */
const webStreamChat: StreamChatTransport = async (token, request, onEvent, options) => {
  const { streamWorkspaceChat } = await import("../workspace/stream");
  return streamWorkspaceChat(token, request, onEvent, options);
};

/**
 * Web 端 REST 请求实现（现有 fetch）
 */
const webRestRequest: RestRequestTransport = async <T>(
  path: string,
  init?: RequestInit,
  token?: string,
): Promise<T> => {
  const { buildApiUrl, ApiError } = await import("../auth/client");

  const headers = new Headers(init?.headers);

  if (!headers.has("Accept")) {
    headers.set("Accept", "application/json");
  }

  if (init?.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }

  const response = await fetch(buildApiUrl(path), {
    ...init,
    cache: "no-store",
    headers,
  });

  if (!response.ok) {
    const raw = await response.text();
    if (!raw.trim()) {
      throw new ApiError(response.status, null, `Request failed with status ${response.status}`);
    }
    try {
      const parsed = JSON.parse(raw) as { error?: string | null; message?: string };
      throw new ApiError(response.status, parsed.error ?? null, parsed.message ?? raw);
    } catch (e) {
      if (e instanceof ApiError) throw e;
      throw new ApiError(response.status, null, raw);
    }
  }

  return (await response.json()) as T;
};

/**
 * 根据运行环境选择流式聊天传输实现
 */
export const streamChat: StreamChatTransport = isTauri() ? tauriStreamChat : webStreamChat;

/**
 * 根据运行环境选择 REST 请求传输实现
 */
export const restRequest: RestRequestTransport = isTauri() ? tauriRestRequest : webRestRequest;

/**
 * 获取 API 基础 URL
 *
 * Web 端：使用环境变量或同源
 * 桌面端：不需要（使用 IPC）
 */
export function getApiBaseUrl(): string {
  if (isTauri()) {
    // 桌面端：使用 IPC，不需要 base URL
    return "";
  }

  // Web 端：使用环境变量或空字符串（同源）
  const configured = process.env.NEXT_PUBLIC_API_BASE_URL?.trim();
  return configured && configured.length > 0 ? configured : "";
}

/**
 * 初始化本地后端（仅桌面端）
 */
export async function initLocalBackend(): Promise<void> {
  if (!isTauri()) {
    return;
  }

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
  if (!isTauri()) {
    return null;
  }

  const { getBackendStatus: getStatus } = await import("./tauri-ipc");
  return getStatus();
}
