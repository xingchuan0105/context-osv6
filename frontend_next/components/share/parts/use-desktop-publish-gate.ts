"use client";

import { useCallback, useEffect, useState } from "react";

import { getCloudSession } from "../../../lib/desktop/tauri-cloud";
import {
  bindDesktopCloudShare,
  getPublishStatus,
  listenPublishProgress,
  publishWorkspace,
  type PublishProgress,
  type PublishStatus,
  type PublishStatusView,
} from "../../../lib/desktop/tauri-publish";
import { isTauri } from "../../../lib/runtime/tauri-ipc";
import { getAppPublicOrigin } from "../../../lib/site-map";

export function useDesktopPublishGate(localWorkspaceId: string) {
  const desktop = isTauri();
  const [statusView, setStatusView] = useState<PublishStatusView | null>(null);
  const [progress, setProgress] = useState<PublishProgress | null>(null);
  const [publishing, setPublishing] = useState(false);
  const [error, setError] = useState("");
  const [sharePublicOrigin, setSharePublicOrigin] = useState("");

  const refresh = useCallback(async () => {
    if (!desktop || !localWorkspaceId) {
      return;
    }
    try {
      const view = await getPublishStatus(localWorkspaceId);
      setStatusView(view);
      setError(view.error ?? "");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [desktop, localWorkspaceId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!desktop) {
      return;
    }
    void getCloudSession()
      .then((session) => {
        const origin = session.cloud_base?.trim();
        setSharePublicOrigin(origin || getAppPublicOrigin());
      })
      .catch(() => {
        setSharePublicOrigin(getAppPublicOrigin());
      });
  }, [desktop]);

  useEffect(() => {
    if (!desktop) {
      return;
    }
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listenPublishProgress((event) => {
      if (!cancelled) {
        setProgress(event);
      }
    }).then((stop) => {
      if (cancelled) {
        stop();
        return;
      }
      unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [desktop]);

  const statusKnown = !desktop || statusView !== null;
  const status: PublishStatus = desktop ? (statusView?.status ?? "never") : "ready";
  const ready = !desktop || (statusKnown && status === "ready");
  const cloudWorkspaceId = statusView?.cloud_workspace_id ?? null;

  useEffect(() => {
    bindDesktopCloudShare(desktop && ready);
    return () => bindDesktopCloudShare(false);
  }, [desktop, ready]);

  const onPublish = useCallback(async () => {
    if (!desktop) {
      return;
    }
    setPublishing(true);
    setError("");
    setProgress({ stage: "pack", current: 0, total: 1, message: "" });
    try {
      const view = await publishWorkspace(localWorkspaceId);
      setStatusView(view);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      try {
        const view = await getPublishStatus(localWorkspaceId);
        setStatusView(view);
        if (view.error) {
          setError(view.error);
        }
      } catch {
        setStatusView({ status: "failed", error: message });
      }
    } finally {
      setPublishing(false);
    }
  }, [desktop, localWorkspaceId]);

  return {
    desktop,
    status,
    statusKnown,
    ready,
    cloudWorkspaceId,
    sharePublicOrigin: desktop ? sharePublicOrigin || getAppPublicOrigin() : null,
    shareWorkspaceId: desktop && ready && cloudWorkspaceId ? cloudWorkspaceId : localWorkspaceId,
    queriesEnabled: ready,
    publishing,
    progress,
    error,
    onPublish,
    onRetryStatus: refresh,
  };
}
