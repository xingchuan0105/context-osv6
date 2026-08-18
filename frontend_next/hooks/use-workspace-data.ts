import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { ApiError } from "../lib/auth/client";
import { useAuth } from "../lib/auth/context";
import { createWorkspace } from "../lib/dashboard/client";
import { getDefaultWorkspaceTitle, markDefaultWorkspaceTitleUsed } from "../lib/dashboard/default-title";
import { useUiPreferences } from "../lib/ui-preferences";
import {
  deleteWorkspaceSession,
  getWorkspace,
  listWorkspaceSessions,
  updateWorkspace,
  updateWorkspaceSession,
} from "../lib/workspace/client";
import type { Workspace } from "../lib/workspace/client";
import type { WorkspaceSession } from "../lib/workspace/model";
import { desktopAppHref } from "../lib/runtime/desktop-app-href";

function todayKey() {
  return new Date().toISOString().slice(0, 10);
}

function getWorkspaceLoadErrorMessage(error: unknown, locale: string) {
  if (error instanceof ApiError && error.status === 404) {
    return locale === "zh-CN"
      ? `当前工作区在后端不存在：${error.message}`
      : `This workspace does not exist on the current backend: ${error.message}`;
  }
  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }
  return locale === "zh-CN"
    ? "当前工作区加载失败，暂时无法继续对话。"
    : "Unable to load this workspace right now.";
}

function pickActiveSessionId(args: {
  preferredOk: string | null;
  current: string | null;
  sessions: WorkspaceSession[];
  bootstrapped: boolean;
}): { next: string | null; bootstrapped: boolean } {
  const { preferredOk, current, sessions, bootstrapped } = args;
  // URL / Cmd+K deep-link always wins when the id is still in the list.
  if (preferredOk) {
    return { next: preferredOk, bootstrapped: true };
  }
  // First successful load for this workspace: default to newest list head.
  if (!bootstrapped) {
    return { next: sessions[0]?.id ?? null, bootstrapped: true };
  }
  // After bootstrap: keep intentional new-thread (null) and valid selection.
  // Clearing `?session=` must not snap back to sessions[0].
  if (current === null) {
    return { next: null, bootstrapped: true };
  }
  if (sessions.some((session) => session.id === current)) {
    return { next: current, bootstrapped: true };
  }
  return { next: sessions[0]?.id ?? null, bootstrapped: true };
}

export function useWorkspaceData(
  workspaceId: string,
  options?: { preferredSessionId?: string | null },
) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const preferredSessionId = options?.preferredSessionId?.trim() || null;
  const sessionBootstrapRef = useRef(false);

  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [workspaceTitleDraft, setWorkspaceTitleDraft] = useState("");
  const [sessions, setSessions] = useState<WorkspaceSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [workspaceLoadError, setWorkspaceLoadError] = useState("");
  const [renameSessionTarget, setRenameSessionTarget] = useState<WorkspaceSession | null>(null);
  const [renameSessionTitle, setRenameSessionTitle] = useState("");
  const [renameSubmitting, setRenameSubmitting] = useState(false);

  useEffect(() => {
    sessionBootstrapRef.current = false;
    setActiveSessionId(null);
    setWorkspace(null);
    setSessions([]);
    setWorkspaceLoadError("");
  }, [workspaceId]);

  useEffect(() => {
    if (!auth.initialized || !auth.token) return;
    let cancelled = false;

    async function load() {
      try {
        const [ws, sess] = await Promise.all([
          getWorkspace(auth.token!, workspaceId),
          listWorkspaceSessions(auth.token!, workspaceId),
        ]);
        if (cancelled) return;
        setWorkspaceLoadError("");
        setWorkspace(ws.workspace);
        setWorkspaceTitleDraft(ws.workspace.title || ws.workspace.name);
        setSessions(sess.sessions);
        // preferredSessionId is applied in a separate effect so writing
        // `?session=` after selection does not re-fetch the workspace.
        const preferredOk =
          preferredSessionId &&
          sess.sessions.some((session) => session.id === preferredSessionId)
            ? preferredSessionId
            : null;
        setActiveSessionId((cur) => {
          const picked = pickActiveSessionId({
            preferredOk,
            current: cur,
            sessions: sess.sessions,
            bootstrapped: sessionBootstrapRef.current,
          });
          sessionBootstrapRef.current = picked.bootstrapped;
          return picked.next;
        });
      } catch (error) {
        if (cancelled) return;
        setWorkspace(null);
        setWorkspaceTitleDraft("");
        setSessions([]);
        setActiveSessionId(null);
        sessionBootstrapRef.current = false;
        setWorkspaceLoadError(getWorkspaceLoadErrorMessage(error, locale));
      }
    }

    void load();
    return () => { cancelled = true; };
    // preferredSessionId intentionally omitted — see effect below.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- deep-link applied separately
  }, [auth.initialized, auth.token, workspaceId, locale]);

  // Cmd+K / external deep-link: switch thread when `?session=` changes without reload.
  useEffect(() => {
    if (!preferredSessionId || sessions.length === 0) {
      return;
    }
    if (!sessions.some((session) => session.id === preferredSessionId)) {
      return;
    }
    setActiveSessionId((cur) => (cur === preferredSessionId ? cur : preferredSessionId));
  }, [preferredSessionId, sessions]);

  useEffect(() => {
    setRenameSessionTarget(null);
    setRenameSessionTitle("");
  }, [workspaceId]);

  const reloadSessions = useCallback(async (preferredSessionId?: string | null) => {
    if (!auth.token) return;
    const response = await listWorkspaceSessions(auth.token, workspaceId);
    setSessions(response.sessions);
    setActiveSessionId((current) => {
      // Explicit preferred id (including mid-stream new session assignment).
      if (preferredSessionId != null) {
        if (response.sessions.some((s) => s.id === preferredSessionId)) {
          return preferredSessionId;
        }
        // Session list may lag creation by a tick — still select the preferred id
        // so the chat pane does not snap back to an older thread.
        return preferredSessionId;
      }
      // preferredSessionId === null means "new thread": keep null, do not fall back
      // to sessions[0] (that used to wipe the live progress card on first send).
      if (preferredSessionId === null) {
        return null;
      }
      // undefined: keep current if still present, else first session.
      if (current && response.sessions.some((s) => s.id === current)) {
        return current;
      }
      return response.sessions[0]?.id ?? null;
    });
  }, [auth.token, workspaceId]);

  const saveWorkspaceTitle = useCallback(async () => {
    if (!auth.token || !workspace) return;
    const nextTitle = workspaceTitleDraft.trim();
    if (!nextTitle) return;
    const response = await updateWorkspace(auth.token, workspaceId, {
      name: nextTitle,
      description: workspace.description,
    });
    localStorage.setItem("avrag.workspace-renamed.v1", String(Date.now()));
    setWorkspace(response.workspace);
    setWorkspaceTitleDraft(response.workspace.title || response.workspace.name);
  }, [auth.token, workspace, workspaceId, workspaceTitleDraft]);

  const createWorkspaceFlow = useCallback(async () => {
    if (!auth.token) return;
    const today = todayKey();
    const title = getDefaultWorkspaceTitle(locale, today);
    const response = await createWorkspace(auth.token, { name: title, description: "" });
    markDefaultWorkspaceTitleUsed(locale, today);
    router.push(desktopAppHref(`/dashboard/${response.workspace.workspace_id}`));
  }, [auth.token, locale, router]);

  const startNewThread = useCallback(() => {
    setActiveSessionId(null);
  }, []);

  const toggleSessionPin = useCallback(async (session: WorkspaceSession) => {
    if (!auth.token) return;
    const updated = await updateWorkspaceSession(auth.token, session.id, { pinned: !session.pinned });
    setSessions((cur) => cur.map((s) => (s.id === updated.id ? updated : s)));
  }, [auth.token]);

  const renameSession = useCallback((session: WorkspaceSession) => {
    setRenameSessionTarget(session);
    setRenameSessionTitle(session.title ?? "");
  }, []);

  const dismissRename = useCallback(() => {
    setRenameSessionTarget(null);
    setRenameSessionTitle("");
  }, []);

  const submitRenameSession = useCallback(async (): Promise<boolean> => {
    if (!auth.token || !renameSessionTarget || renameSubmitting) return false;
    setRenameSubmitting(true);
    try {
      const updated = await updateWorkspaceSession(auth.token, renameSessionTarget.id, {
        title: renameSessionTitle.trim(),
      });
      setSessions((cur) => cur.map((s) => (s.id === updated.id ? updated : s)));
      setRenameSessionTarget(null);
      setRenameSessionTitle("");
      return true;
    } catch (error) {
      // Keep the dialog open so the caller can surface an inline error.
      console.error(error);
      return false;
    } finally {
      setRenameSubmitting(false);
    }
  }, [auth.token, renameSessionTarget, renameSessionTitle, renameSubmitting]);

  const removeSession = useCallback(async (session: WorkspaceSession): Promise<boolean> => {
    if (!auth.token) return false;
    try {
      await deleteWorkspaceSession(auth.token, session.id);
    } catch (error) {
      console.error(error);
      return false;
    }
    setRenameSessionTarget((cur) => {
      if (cur?.id === session.id) {
        setRenameSessionTitle("");
        return null;
      }
      return cur;
    });
    setSessions((cur) => {
      const next = cur.filter((s) => s.id !== session.id);
      setActiveSessionId((active) => {
        if (active !== session.id) return active;
        return next[0]?.id ?? null;
      });
      return next;
    });
    return true;
  }, [auth.token]);

  return {
    workspace,
    setWorkspace,
    workspaceTitleDraft,
    setWorkspaceTitleDraft,
    sessions,
    setSessions,
    activeSessionId,
    setActiveSessionId,
    workspaceLoadError,
    renameSessionTarget,
    renameSessionTitle,
    setRenameSessionTitle,
    renameSubmitting,
    reloadSessions,
    saveWorkspaceTitle,
    createWorkspaceFlow,
    startNewThread,
    toggleSessionPin,
    renameSession,
    dismissRename,
    submitRenameSession,
    removeSession,
  };
}
