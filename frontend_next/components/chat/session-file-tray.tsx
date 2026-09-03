"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { SessionFileRow } from "../../lib/contracts/generated";
import {
  completeChatSessionFileUpload,
  createChatSessionFileUpload,
  createPersonalChatSession,
  deleteChatSessionFile,
  listChatSessionFiles,
  reindexChatSessionFile,
} from "../../lib/chat/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./session-file-tray.module.css";

const SUPPORTED_UPLOAD_ACCEPT =
  ".pdf,.doc,.docx,.ppt,.pptx,.xls,.xlsx,.txt,.md,.csv,.json,.toml,.yaml,.yml,.rst";

type TrayFileStatus = "uploading" | "parsing" | "ready" | "failed";

function trayStatus(status: string): TrayFileStatus {
  switch (status) {
    case "completed":
      return "ready";
    case "failed":
    case "upload_invalid":
      return "failed";
    case "pending":
    case "enqueueing":
      return "uploading";
    default:
      // queued / processing / anything in flight.
      return "parsing";
  }
}

const NON_READY_BLOCKING_STATUSES = new Set(["pending", "enqueueing", "queued", "processing"]);

type SessionFileTrayProps = {
  token: string;
  sessionId: string | null;
  disabled: boolean;
  onSessionChange?: (sessionId: string) => void;
  /** True while any explicitly attached file has not reached a terminal state. */
  onBlockedChange?: (blocked: boolean) => void;
  /** Number of ready files — the canvas auto-attaches retrieval on >0. */
  onReadyCountChange?: (readyCount: number) => void;
};

/**
 * Chat-first W2b: per-Conversation file tray. First upload on a fresh
 * conversation creates the Conversation idempotently (design §7.1); every file
 * carries its own uploading → parsing → ready | failed state with retry/remove.
 */
export function SessionFileTray({
  token,
  sessionId,
  disabled,
  onSessionChange,
  onBlockedChange,
  onReadyCountChange,
}: SessionFileTrayProps) {
  const { locale } = useUiPreferences();
  const [files, setFiles] = useState<SessionFileRow[]>([]);
  const [actionError, setActionError] = useState(false);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  /**
   * Permanent tombstone: bindings removed from this session. A DELETE that
   * succeeded must stay dead for the lifetime of the session — any poll or
   * reconcile GET issued before the deletion can still return afterwards and
   * would otherwise resurrect the row (review round-6: the previous
   * in-flight-only guard cleared the tombstone before the last stale GET
   * landed). Cleared only when the session changes.
   */
  const removedRef = useRef<Set<string>>(new Set());

  const refresh = useCallback(async () => {
    if (!token || !sessionId) {
      return;
    }
    try {
      const next = await listChatSessionFiles(token, sessionId);
      setFiles(next.filter((row) => !removedRef.current.has(row.binding_id)));
    } catch {
      // Keep the last known state; the next poll retries.
    }
  }, [sessionId, token]);

  useEffect(() => {
    setFiles([]);
    setActionError(false);
    removedRef.current.clear();
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId) {
      return;
    }
    void refresh();
  }, [sessionId, refresh]);

  // Poll while anything is in flight so the tray reaches ready/failed on its own.
  useEffect(() => {
    const inFlight = files.some((file) => NON_READY_BLOCKING_STATUSES.has(file.status));
    if (!inFlight || !sessionId || !token) {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
      return;
    }
    if (pollRef.current) {
      return;
    }
    pollRef.current = setInterval(() => void refresh(), 2000);
    return () => {
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [files, refresh, sessionId, token]);

  const blocked = files.some((file) => NON_READY_BLOCKING_STATUSES.has(file.status));
  const readyCount = files.filter((file) => file.status === "completed").length;

  useEffect(() => {
    onBlockedChange?.(blocked);
  }, [blocked, onBlockedChange]);

  useEffect(() => {
    onReadyCountChange?.(readyCount);
  }, [readyCount, onReadyCountChange]);

  const ensureSession = useCallback(async (): Promise<string | null> => {
    if (sessionId) {
      return sessionId;
    }
    if (!token) {
      return null;
    }
    // First upload creates the Conversation explicitly (design §7.1).
    const created = await createPersonalChatSession(token);
    onSessionChange?.(created.id);
    return created.id;
  }, [onSessionChange, sessionId, token]);

  const handleFilesPicked = useCallback(
    async (picked: FileList | null) => {
      if (!picked || picked.length === 0 || disabled || busy) {
        return;
      }
    setBusy(true);
    setActionError(false);
    try {
      let activeSessionId = sessionId;
      for (const file of Array.from(picked)) {
        activeSessionId = await ensureSession();
        if (!activeSessionId || !token) {
          return;
        }
        const upload = await createChatSessionFileUpload(token, activeSessionId, {
          filename: file.name,
          file_size: file.size,
          mime_type: file.type || "application/octet-stream",
        });
        const putResponse = await fetch(upload.upload_url, {
          method: "PUT",
          body: file,
        });
        if (!putResponse.ok) {
          setActionError(true);
          return;
        }
        await completeChatSessionFileUpload(token, upload.document_id);
      }
      if (activeSessionId) {
        await refresh();
      }
    } catch {
      setActionError(true);
    } finally {
        setBusy(false);
        if (inputRef.current) {
          inputRef.current.value = "";
        }
      }
    },
    [busy, disabled, ensureSession, refresh, sessionId, token],
  );

  const handleRetry = useCallback(
    async (file: SessionFileRow) => {
      if (!token || !sessionId) {
        return;
      }
      await reindexChatSessionFile(token, file.document_id);
      await refresh();
    },
    [refresh, sessionId, token],
  );

  const handleRemove = useCallback(
    async (file: SessionFileRow) => {
      if (!token || !sessionId) {
        return;
      }
      // Permanent tombstone + optimistic removal: every future read filters
      // this binding, so neither a stale poll nor a late reconcile GET can
      // resurrect it. On DELETE failure the tombstone is lifted and the row
      // is rolled back — swallowing the error would fork UI from server
      // truth when the reconcile refresh also fails (review round-5 S3).
      removedRef.current.add(file.binding_id);
      setFiles((prev) => prev.filter((row) => row.binding_id !== file.binding_id));
      try {
        await deleteChatSessionFile(token, sessionId, file.binding_id);
      } catch {
        removedRef.current.delete(file.binding_id);
        setFiles((prev) =>
          prev.some((row) => row.binding_id === file.binding_id) ? prev : [file, ...prev],
        );
        setActionError(true);
        return;
      }
      await refresh();
    },
    [refresh, sessionId, token],
  );

  const statusKey = (status: TrayFileStatus) => `chat.fileStatus.${status}` as const;

  return (
    <div className={styles.tray} data-testid="session-file-tray">
      <input
        accept={SUPPORTED_UPLOAD_ACCEPT}
        hidden
        multiple
        onChange={(event) => void handleFilesPicked(event.target.files)}
        ref={inputRef}
        type="file"
      />
      <div className={styles.trayRow}>
        <button
          className={styles.attachButton}
          disabled={disabled || busy}
          onClick={() => inputRef.current?.click()}
          type="button"
        >
          {formatUiMessage(locale, "chat.attachFile")}
        </button>
        {files.length > 0 && (
          <span className={styles.trayLabel}>
            {formatUiMessage(locale, "chat.fileTrayLabel")} · {files.length}
          </span>
        )}
      </div>
      {actionError && (
        <p className={styles.error} role="alert">
          {formatUiMessage(locale, "chat.fileActionFailed")}
        </p>
      )}
      {blocked && <p className={styles.blockedHint}>{formatUiMessage(locale, "chat.fileBlockedHint")}</p>}
      {files.length > 0 && (
        <ul className={styles.fileList}>
          {files.map((file) => {
            const status = trayStatus(file.status);
            return (
              <li className={styles.fileItem} data-status={status} key={file.binding_id}>
                <span className={styles.fileName}>{file.file_name}</span>
                <span className={styles.fileStatus}>{formatUiMessage(locale, statusKey(status))}</span>
                {status === "failed" && (
                  <button
                    className={styles.fileAction}
                    onClick={() => void handleRetry(file)}
                    type="button"
                  >
                    {formatUiMessage(locale, "chat.fileRetry")}
                  </button>
                )}
                <button
                  className={styles.fileAction}
                  onClick={() => void handleRemove(file)}
                  type="button"
                >
                  {formatUiMessage(locale, "chat.fileRemove")}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
