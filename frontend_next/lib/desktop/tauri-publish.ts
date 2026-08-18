/**
 * Desktop workspace publish IPC (ADR-0010 B3b).
 *
 * Publish talks to the local sidecar for export and the cloud API (session JWT)
 * for sessions/parts/commit. Share after `ready` also uses `cloudApiCall`.
 */

import { setShareRequestOverride, type ShareRequestFn } from "../share/client";

export type PublishStatus = "never" | "publishing" | "ready" | "failed";

export type PublishStatusView = {
  status: PublishStatus;
  cloud_workspace_id?: string | null;
  last_published_at?: string | null;
  error?: string | null;
  expected_parts?: number | null;
};

export type PublishProgress = {
  stage: string;
  current: number;
  total: number;
  message: string;
};

function mapIpcError(error: unknown): Error {
  if (error instanceof Error) {
    return error;
  }
  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      if (
        parsed &&
        typeof parsed === "object" &&
        "message" in parsed &&
        typeof (parsed as { message: unknown }).message === "string"
      ) {
        return new Error((parsed as { message: string }).message);
      }
    } catch {
      return new Error(error);
    }
    return new Error(error);
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return new Error(message);
    }
  }
  return new Error(String(error));
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw mapIpcError(error);
  }
}

export async function getPublishStatus(localWorkspaceId: string): Promise<PublishStatusView> {
  return invoke<PublishStatusView>("get_publish_status", {
    localWorkspaceId,
  });
}

export async function publishWorkspace(localWorkspaceId: string): Promise<PublishStatusView> {
  return invoke<PublishStatusView>("publish_workspace", {
    localWorkspaceId,
  });
}

export async function cloudApiCall<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  return invoke<T>("cloud_api_call", { method, path, body: body ?? null });
}

export const cloudShareRequest: ShareRequestFn = async (path, init = {}, _token) => {
  const method = (init.method || "GET").toUpperCase();
  let body: unknown;
  if (typeof init.body === "string" && init.body.trim()) {
    body = JSON.parse(init.body);
  }
  return cloudApiCall(method, path, body);
};

export function bindDesktopCloudShare(enabled: boolean) {
  setShareRequestOverride(enabled ? cloudShareRequest : null);
}

export async function listenPublishProgress(
  onProgress: (progress: PublishProgress) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<PublishProgress>("workspace-publish-progress", (event) => {
    onProgress(event.payload);
  });
  return unlisten;
}
