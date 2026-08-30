"use client";

/**
 * 桌面端自动更新（设置 → 关于）。
 *
 * 走 tauri-plugin-updater：更新源 `https://app.contextlm.top/releases/desktop/updates.json`，
 * 安装包 minisign 签名（pubkey 内置于 tauri.conf.json）。两段式：先 checkForUpdate，
 * 用户确认后再 downloadAndInstallUpdate（NSIS 静默安装 + 自动重启）。
 * 非 Tauri 环境（网页 / SSR）不可用，check 返回 upToDate。
 */

import { isTauri } from "@/lib/runtime/tauri-ipc";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string }
  | { kind: "downloading"; progress: number }
  | { kind: "installing" }
  | { kind: "error"; message: string };

type UpdaterUpdate = Awaited<ReturnType<typeof import("@tauri-apps/plugin-updater").check>>;

/** check() 与 downloadAndInstall() 之间持有的更新对象（单窗口单次更新流，无需队列）。 */
let pendingUpdate: NonNullable<UpdaterUpdate> | null = null;

/** 更新能力是否存在（桌面环境且插件可加载）。非桌面环境永不支持。 */
export async function desktopUpdateSupported(): Promise<boolean> {
  if (typeof window === "undefined" || !isTauri()) return false;
  try {
    await import("@tauri-apps/plugin-updater");
    return true;
  } catch {
    return false;
  }
}

/** 第一步：检查更新。返回 available（含新版本号）/ upToDate / error。 */
export async function checkForUpdate(): Promise<UpdateState> {
  pendingUpdate = null;
  if (typeof window === "undefined" || !isTauri()) return { kind: "upToDate" };
  const { check } = await import("@tauri-apps/plugin-updater");
  try {
    const update = await check();
    if (!update) return { kind: "upToDate" };
    pendingUpdate = update;
    return { kind: "available", version: update.version };
  } catch (error) {
    return {
      kind: "error",
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * 第二步：下载并安装已发现的更新，完成后应用自动重启（本函数不会正常返回后续状态——
 * 进程退出；失败返回 error 供 UI 展示）。
 */
export async function downloadAndInstallUpdate(
  onProgress?: (percent: number) => void
): Promise<UpdateState> {
  const update = pendingUpdate;
  if (!update) return { kind: "upToDate" };
  const { relaunch } = await import("@tauri-apps/plugin-process");

  let downloaded = 0;
  let contentLength: number | null = null;
  try {
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          contentLength = event.data.contentLength ?? null;
          onProgress?.(0);
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          if (contentLength && contentLength > 0) {
            onProgress?.(Math.min(100, Math.round((downloaded / contentLength) * 100)));
          }
          break;
        case "Finished":
          onProgress?.(100);
          break;
      }
    });
  } catch (error) {
    return {
      kind: "error",
      message: error instanceof Error ? error.message : String(error),
    };
  }

  // Windows NSIS 安装器启动前退出当前进程（插件文档约定）。
  void relaunch();
  return { kind: "installing" };
}
