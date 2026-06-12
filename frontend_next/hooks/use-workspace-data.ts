import { useCallback, useEffect, useState } from "react";
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

export function useWorkspaceData(workspaceId: string) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();

  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [workspaceTitleDraft, setWorkspaceTitleDraft] = useState("");
  const [sessions, setSessions] = useState<WorkspaceSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [workspaceLoadError, setWorkspaceLoadError] = useState("");
  const [renameSessionTarget, setRenameSessionTarget] = useState<WorkspaceSession | null>(null);
  const [renameSessionTitle, setRenameSessionTitle] = useState("");

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
        setActiveSessionId((cur) => cur ?? sess.sessions[0]?.id ?? null);
      } catch (error) {
        if (cancelled) return;
        setWorkspace(null);
        setWorkspaceTitleDraft("");
        setSessions([]);
        setActiveSessionId(null);
        setWorkspaceLoadError(getWorkspaceLoadErrorMessage(error, locale));
      }
    }

    void load();
    return () => { cancelled = true; };
  }, [auth.initialized, auth.token, workspaceId]);

  useEffect(() => {
    setRenameSessionTarget(null);
    setRenameSessionTitle("");
  }, [workspaceId]);

  const reloadSessions = useCallback(async (preferredSessionId?: string | null) => {
    if (!auth.token) return;
    const response = await listWorkspaceSessions(auth.token, workspaceId);
    setSessions(response.sessions);
    setActiveSessionId((current) => {
      const next = preferredSessionId ?? current;
      if (next && response.sessions.some((s) => s.id === next)) return next;
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
    router.push(`/dashboard/${response.workspace.workspace_id}`);
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

  const submitRenameSession = useCallback(async () => {
    if (!auth.token || !renameSessionTarget) return;
    const updated = await updateWorkspaceSession(auth.token, renameSessionTarget.id, {
      title: renameSessionTitle.trim(),
    });
    setSessions((cur) => cur.map((s) => (s.id === updated.id ? updated : s)));
    setRenameSessionTarget(null);
    setRenameSessionTitle("");
  }, [auth.token, renameSessionTarget, renameSessionTitle]);

  const removeSession = useCallback(async (session: WorkspaceSession) => {
    if (!auth.token) return;
    await deleteWorkspaceSession(auth.token, session.id);
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
