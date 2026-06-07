"use client";

import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type TouchEvent as ReactTouchEvent,
} from "react";
import { useRouter } from "next/navigation";

import { ApiError } from "../../lib/auth/client";
import { useAuth } from "../../lib/auth/context";
import { billingApi } from "../../lib/billing/api";
import type { UsageWindowBucket, UsageWindowResponse } from "../../lib/billing/api";
import { isPricingRevampEnabledClient } from "../../lib/billing/featureFlag";
import { createWorkspace } from "../../lib/dashboard/client";
import {
  getDefaultWorkspaceTitle,
  markDefaultWorkspaceTitleUsed,
} from "../../lib/dashboard/default-title";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  deleteWorkspaceSession,
  getWorkspace,
  listWorkspaceSessions,
  updateWorkspace,
  updateWorkspaceSession,
} from "../../lib/workspace/client";
import type { Workspace } from "../../lib/workspace/client";
import type { WorkspaceSession, WorkspaceWebSourcesRequest } from "../../lib/workspace/model";
import { DEFAULT_WORKSPACE_UI_STATE, useWorkspaceUi } from "../../lib/workspace/ui-store";
import styles from "./workspace-shell.module.css";
import { WorkspaceChatPane } from "./workspace-chat-pane";
import { WorkspaceCitationModal } from "./workspace-citation-modal";
import { WorkspaceHistoryPane } from "./workspace-history-pane";
import { WorkspaceRightRail } from "./workspace-right-rail";
import { WorkspaceTopBar } from "./workspace-top-bar";
import { UsageWarningToast } from "../billing/UsageWarningToast";

function todayKey() {
  return new Date().toISOString().slice(0, 10);
}

function usageWindowPressure(bucket: UsageWindowBucket): number {
  if (bucket.limit <= 0) {
    return 0;
  }
  return bucket.used / bucket.limit;
}

function useIsMobile() {
  const [isMobile, setIsMobile] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }

    const mediaQuery = window.matchMedia("(max-width: 767px)");
    const handleChange = (event: MediaQueryListEvent) => setIsMobile(event.matches);

    setIsMobile(mediaQuery.matches);

    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", handleChange);

      return () => {
        mediaQuery.removeEventListener("change", handleChange);
      };
    }

    mediaQuery.addListener(handleChange);

    return () => {
      mediaQuery.removeListener(handleChange);
    };
  }, []);

  return isMobile;
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

export function WorkspaceSurface({ workspaceId }: { workspaceId: string }) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const isMobile = useIsMobile();
  const workspaceUi = useWorkspaceUi(workspaceId);
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [workspaceTitleDraft, setWorkspaceTitleDraft] = useState("");
  const [renameSessionTarget, setRenameSessionTarget] = useState<WorkspaceSession | null>(null);
  const [renameSessionTitle, setRenameSessionTitle] = useState("");
  const [sessions, setSessions] = useState<WorkspaceSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [workspaceLoadError, setWorkspaceLoadError] = useState("");
  const [activeWebSources, setActiveWebSources] =
    useState<WorkspaceWebSourcesRequest | null>(null);
  const [usageWarning, setUsageWarning] = useState<{
    threshold: 80 | 95;
    percentage: number;
    windowType: "5h" | "7d";
    data: UsageWindowResponse;
  } | null>(null);
  const activeResizeCleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    if (!auth.initialized || !auth.token) {
      return;
    }

    let cancelled = false;

    async function loadWorkspace() {
      const [workspaceResponse, sessionResponse] = await Promise.all([
        getWorkspace(auth.token!, workspaceId),
        listWorkspaceSessions(auth.token!, workspaceId),
      ]);

      if (cancelled) {
        return;
      }

      setWorkspaceLoadError("");
      setWorkspace(workspaceResponse.workspace);
      setWorkspaceTitleDraft(workspaceResponse.workspace.title || workspaceResponse.workspace.name);
      setSessions(sessionResponse.sessions);
      setActiveSessionId((current) => current ?? sessionResponse.sessions[0]?.id ?? null);
    }

    void loadWorkspace().catch((error) => {
      if (!cancelled) {
        setWorkspaceLoadError(getWorkspaceLoadErrorMessage(error, locale));
        setWorkspace(null);
        setSessions([]);
        setActiveSessionId(null);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [auth.initialized, auth.token, locale, workspaceId]);

  useEffect(() => {
    if (!auth.initialized || !auth.token || !auth.user?.id || !isPricingRevampEnabledClient()) {
      return;
    }

    let cancelled = false;

    void billingApi
      .getUsageWindow()
      .then((usageWindow) => {
        if (cancelled) {
          return;
        }

        if (usageWindow.hard_limit_hit.rolling_5h || usageWindow.hard_limit_hit.rolling_7d) {
          let reason: "5h" | "7d";
          if (usageWindow.hard_limit_hit.rolling_5h && usageWindow.hard_limit_hit.rolling_7d) {
            reason =
              usageWindowPressure(usageWindow.rolling_5h) >=
              usageWindowPressure(usageWindow.rolling_7d)
                ? "5h"
                : "7d";
          } else {
            reason = usageWindow.hard_limit_hit.rolling_5h ? "5h" : "7d";
          }
          router.push(`/upgrade/paywall?reason=${reason}`);
          return;
        }

        if (usageWindow.soft_limit_hit.rolling_5h) {
          setUsageWarning({
            threshold: usageWindow.rolling_5h.percentage >= 95 ? 95 : 80,
            percentage: usageWindow.rolling_5h.percentage,
            windowType: "5h",
            data: usageWindow,
          });
          return;
        }

        if (usageWindow.soft_limit_hit.rolling_7d) {
          setUsageWarning({
            threshold: usageWindow.rolling_7d.percentage >= 95 ? 95 : 80,
            percentage: usageWindow.rolling_7d.percentage,
            windowType: "7d",
            data: usageWindow,
          });
        }
      })
      .catch(() => {
        // Ignore transient billing probe failures; workspace remains usable.
      });

    return () => {
      cancelled = true;
    };
  }, [auth.initialized, auth.token, auth.user?.id, router]);

  useEffect(() => {
    setRenameSessionTarget(null);
    setRenameSessionTitle("");
  }, [workspaceId]);

  useEffect(() => {
    if (!isMobile) {
      return;
    }

    if (
      workspaceUi.historyRailOpen === DEFAULT_WORKSPACE_UI_STATE.historyRailOpen &&
      workspaceUi.rightRailOpen === DEFAULT_WORKSPACE_UI_STATE.rightRailOpen
    ) {
      workspaceUi.setHistoryRailOpen(false);
      workspaceUi.setRightRailOpen(false);
    }
  }, [isMobile, workspaceUi]);

  useEffect(() => {
    if (!isMobile || (!workspaceUi.historyRailOpen && !workspaceUi.rightRailOpen)) {
      return;
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") {
        return;
      }

      workspaceUi.setHistoryRailOpen(false);
      workspaceUi.setRightRailOpen(false);
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [isMobile, workspaceUi]);

  useEffect(
    () => () => {
      activeResizeCleanupRef.current?.();
    },
    [],
  );

  async function reloadSessions(preferredSessionId?: string | null) {
    if (!auth.token) {
      return;
    }

    const response = await listWorkspaceSessions(auth.token, workspaceId);
    setSessions(response.sessions);

    setActiveSessionId((current) => {
      const nextActive = preferredSessionId ?? current;

      if (nextActive && response.sessions.some((session) => session.id === nextActive)) {
        return nextActive;
      }

      return response.sessions[0]?.id ?? null;
    });
  }

  async function saveWorkspaceTitle() {
    if (!auth.token || !workspace) {
      return;
    }

    const nextTitle = workspaceTitleDraft.trim();
    if (!nextTitle) {
      return;
    }

    const response = await updateWorkspace(auth.token, workspaceId, {
      name: nextTitle,
      description: workspace.description,
    });

    localStorage.setItem("avrag.workspace-renamed.v1", String(Date.now()));

    setWorkspace(response.workspace);
    setWorkspaceTitleDraft(response.workspace.title || response.workspace.name);
  }

  async function createWorkspaceFlow() {
    if (!auth.token) {
      return;
    }

    const title = getDefaultWorkspaceTitle(locale, todayKey());
    const response = await createWorkspace(auth.token, {
      name: title,
      description: "",
    });

    markDefaultWorkspaceTitleUsed(locale, todayKey());
    router.push(`/dashboard/${response.workspace.workspace_id}`);
  }

  async function startNewThread() {
    setActiveSessionId(null);
    workspaceUi.setActiveCitation(null);
    workspaceUi.setFocusedSourceId(null);
  }

  async function toggleSessionPin(session: WorkspaceSession) {
    if (!auth.token) {
      return;
    }

    const updated = await updateWorkspaceSession(auth.token, session.id, {
      pinned: !session.pinned,
    });

    setSessions((current) => current.map((item) => (item.id === updated.id ? updated : item)));
  }

  function renameSession(session: WorkspaceSession) {
    setRenameSessionTarget(session);
    setRenameSessionTitle(session.title ?? "");
  }

  async function submitRenameSession() {
    if (!auth.token || !renameSessionTarget) {
      return;
    }

    const updated = await updateWorkspaceSession(auth.token, renameSessionTarget.id, {
      title: renameSessionTitle.trim(),
    });

    setSessions((current) => current.map((item) => (item.id === updated.id ? updated : item)));
    setRenameSessionTarget(null);
    setRenameSessionTitle("");
  }

  async function removeSession(session: WorkspaceSession) {
    if (!auth.token) {
      return;
    }

    await deleteWorkspaceSession(auth.token, session.id);
    if (renameSessionTarget?.id === session.id) {
      setRenameSessionTarget(null);
      setRenameSessionTitle("");
    }
    setSessions((current) => {
      const nextSessions = current.filter((item) => item.id !== session.id);

      setActiveSessionId((currentActive) => {
        if (currentActive !== session.id) {
          return currentActive;
        }

        return nextSessions[0]?.id ?? null;
      });

      return nextSessions;
    });
  }

  const historyPane = (
    <WorkspaceHistoryPane
      activeSessionId={activeSessionId}
      onDeleteSession={(session) => void removeSession(session)}
      onNewThread={() => void startNewThread()}
      onRenameSession={renameSession}
      onRequestClose={() => workspaceUi.setHistoryRailOpen(false)}
      onSelectSession={setActiveSessionId}
      onTogglePinSession={(session) => void toggleSessionPin(session)}
      sessions={sessions}
    />
  );

  const rightRail = (
    <WorkspaceRightRail
      activeWebSources={activeWebSources}
      focusedSourceId={workspaceUi.focusedSourceId}
      onCloseWebSources={() => setActiveWebSources(null)}
      onSelectedSourceIdsChange={workspaceUi.setSelectedSourceIds}
      selectedSourceIds={workspaceUi.selectedSourceIds}
      workspaceId={workspaceId}
    />
  );

  function beginDesktopResize(side: "history" | "right", startX: number, mode: "mouse" | "pointer" | "touch") {
    if (isMobile || activeResizeCleanupRef.current) {
      return;
    }
    const startWidth = side === "history" ? workspaceUi.historyRailWidth : workspaceUi.rightRailWidth;

    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    function applyDelta(clientX: number) {
      const delta = clientX - startX;

      if (side === "history") {
        workspaceUi.setHistoryRailWidth(startWidth + delta);
        return;
      }

      workspaceUi.setRightRailWidth(startWidth - delta);
    }

    function finishResize() {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", finishResize);
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", finishResize);
      window.removeEventListener("pointercancel", finishResize);
      window.removeEventListener("touchmove", handleTouchMove);
      window.removeEventListener("touchend", finishResize);
      window.removeEventListener("touchcancel", finishResize);
      activeResizeCleanupRef.current = null;
    }

    function handleMouseMove(moveEvent: MouseEvent) {
      applyDelta(moveEvent.clientX);
    }

    function handlePointerMove(moveEvent: PointerEvent) {
      applyDelta(moveEvent.clientX);
    }

    function handleTouchMove(moveEvent: TouchEvent) {
      const touch = moveEvent.touches[0];
      if (!touch) {
        return;
      }

      moveEvent.preventDefault();
      applyDelta(touch.clientX);
    }

    activeResizeCleanupRef.current = finishResize;

    if (mode === "pointer") {
      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", finishResize);
      window.addEventListener("pointercancel", finishResize);
      return;
    }

    if (mode === "touch") {
      window.addEventListener("touchmove", handleTouchMove, { passive: false });
      window.addEventListener("touchend", finishResize);
      window.addEventListener("touchcancel", finishResize);
      return;
    }

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", finishResize);
  }

  function startDesktopMouseResize(side: "history" | "right", event: ReactMouseEvent<HTMLDivElement>) {
    event.preventDefault();
    beginDesktopResize(side, event.clientX, "mouse");
  }

  function startDesktopPointerResize(side: "history" | "right", event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    beginDesktopResize(side, event.clientX, "pointer");
  }

  function startDesktopTouchResize(side: "history" | "right", event: ReactTouchEvent<HTMLDivElement>) {
    const touch = event.touches[0];
    if (!touch) {
      return;
    }

    event.preventDefault();
    beginDesktopResize(side, touch.clientX, "touch");
  }

  const bodyStyle = {
    "--workspace-history-rail-width": `${workspaceUi.historyRailWidth}px`,
    "--workspace-right-rail-width": `${workspaceUi.rightRailWidth}px`,
  } as CSSProperties;

  const workspaceUnavailableTitle =
    locale === "zh-CN" ? "当前工作区不可用" : "Workspace unavailable";
  const workspaceUnavailableHint =
    locale === "zh-CN"
      ? "这个页面的 workspace id 当前不在后端数据库里，继续发送消息只会返回错误。请确认你连接的是正确的 API/数据库，或者重新创建一个工作区。"
      : "This workspace id is not present in the current backend database. Sending messages here will only fail. Check that you are connected to the expected API/database, or create a new workspace.";

  return (
    <main className={styles.shell}>
      <WorkspaceTopBar
        onCreateWorkspaceSubmit={() => void createWorkspaceFlow()}
        onSaveWorkspaceTitle={() => void saveWorkspaceTitle()}
        onWorkspaceTitleDraftChange={setWorkspaceTitleDraft}
        workspaceDescription={workspace?.description || ""}
        workspaceId={workspaceId}
        workspaceTitle={workspace?.title || workspace?.name || workspaceId}
        workspaceTitleDraft={workspaceTitleDraft}
      />

      {workspaceLoadError ? (
        <section className={styles.workspaceUnavailable}>
          <div className={styles.workspaceUnavailableCard}>
            <h2 className={styles.workspaceUnavailableTitle}>{workspaceUnavailableTitle}</h2>
            <p className={styles.workspaceUnavailableMessage}>{workspaceLoadError}</p>
            <p className={styles.workspaceUnavailableHint}>{workspaceUnavailableHint}</p>
            <div className={styles.workspaceUnavailableActions}>
              <button className={styles.primaryButton} type="button" onClick={() => void createWorkspaceFlow()}>
                {locale === "zh-CN" ? "新建工作区" : "Create workspace"}
              </button>
            </div>
          </div>
        </section>
      ) : (
        <>
          <div className={styles.body} style={bodyStyle}>
            <aside className={styles.desktopHistoryRail} data-testid="desktop-history-rail">
              {historyPane}
            </aside>

            <div
              aria-label="Resize history rail"
              className={styles.desktopRailResizer}
              onMouseDown={(event) => startDesktopMouseResize("history", event)}
              onPointerDown={(event) => startDesktopPointerResize("history", event)}
              onTouchStart={(event) => startDesktopTouchResize("history", event)}
              role="separator"
            />

            <section className={styles.panePanel}>
              <WorkspaceChatPane
                onFocusSource={(sourceId) => {
                  workspaceUi.setFocusedSourceId(sourceId);
                  if (sourceId) {
                    workspaceUi.setRightRailOpen(true);
                  }
                }}
                onOpenWebSources={(request) => {
                  setActiveWebSources(request);
                  workspaceUi.setRightRailOpen(true);
                  workspaceUi.setActiveCitation(null);
                }}
                onSelectCitation={(request) => {
                  workspaceUi.setActiveCitation(request);
                }}
                onSessionActivity={() => void reloadSessions(activeSessionId)}
                onSessionChange={(sessionId) => {
                  setActiveSessionId(sessionId);
                  workspaceUi.setActiveCitation(null);
                  void reloadSessions(sessionId);
                }}
                selectedSourceIds={workspaceUi.selectedSourceIds}
                sessionId={activeSessionId}
                workspaceId={workspaceId}
              />
            </section>

            <div
              aria-label="Resize right rail"
              className={styles.desktopRailResizer}
              onMouseDown={(event) => startDesktopMouseResize("right", event)}
              onPointerDown={(event) => startDesktopPointerResize("right", event)}
              onTouchStart={(event) => startDesktopTouchResize("right", event)}
              role="separator"
            />

            <aside
              className={styles.desktopRightRail}
              aria-label={formatUiMessage(locale, "workspaceRightRail.label")}
              data-testid="desktop-right-rail"
            >
              {rightRail}
            </aside>
          </div>

          <WorkspaceCitationModal
            citationRequest={workspaceUi.activeCitation}
            onClose={() => {
              workspaceUi.setActiveCitation(null);
            }}
            workspaceId={workspaceId}
          />

          {isMobile && workspaceUi.historyRailOpen ? (
            <div className={styles.mobileOverlay} onClick={() => workspaceUi.setHistoryRailOpen(false)}>
              <div className={styles.mobileScrim} />
              <aside
                className={`${styles.mobileDrawer} ${styles.mobileHistoryDrawer}`}
                data-testid="mobile-history-drawer"
                onClick={(event) => event.stopPropagation()}
              >
                {historyPane}
              </aside>
            </div>
          ) : null}

          {isMobile && workspaceUi.rightRailOpen ? (
            <div className={styles.mobileOverlay} onClick={() => workspaceUi.setRightRailOpen(false)}>
              <div className={styles.mobileScrim} />
              <aside
                aria-label={formatUiMessage(locale, "workspaceRightRail.label")}
                className={`${styles.mobileDrawer} ${styles.mobileRightDrawer}`}
                data-testid="mobile-right-drawer"
                onClick={(event) => event.stopPropagation()}
              >
                {rightRail}
              </aside>
            </div>
          ) : null}
        </>
      )}

      {renameSessionTarget ? (
        <div
          className={styles.modalBackdrop}
          onClick={() => {
            setRenameSessionTarget(null);
            setRenameSessionTitle("");
          }}
        >
          <div
            className={styles.modalCard}
            role="dialog"
            aria-label={formatUiMessage(locale, "workspaceRenameSessionDialogLabel")}
            onClick={(event) => event.stopPropagation()}
          >
            <form
              className={styles.dialogForm}
              onSubmit={(event) => {
                event.preventDefault();
                void submitRenameSession();
              }}
            >
              <div className={styles.dialogField}>
                <label htmlFor="rename-session-title">{formatUiMessage(locale, "workspaceThreadTitleField")}</label>
                <input
                  id="rename-session-title"
                  autoFocus
                  value={renameSessionTitle}
                  onChange={(event) => setRenameSessionTitle(event.target.value)}
                />
              </div>
              <div className={styles.dialogActions}>
                <button
                  className={styles.secondaryButton}
                  type="button"
                  onClick={() => {
                    setRenameSessionTarget(null);
                    setRenameSessionTitle("");
                  }}
                >
                  {formatUiMessage(locale, "commonCancel")}
                </button>
                <button className={styles.primaryButton} type="submit">
                  {formatUiMessage(locale, "workspaceRenameSessionSubmit")}
                </button>
              </div>
            </form>
          </div>
        </div>
      ) : null}

      {usageWarning && auth.user?.id ? (
        <UsageWarningToast
          threshold={usageWarning.threshold}
          percentage={usageWarning.percentage}
          windowType={usageWarning.windowType}
          locale={locale}
          userId={auth.user.id}
          used={
            usageWarning.windowType === "5h"
              ? usageWarning.data.rolling_5h.used
              : usageWarning.data.rolling_7d.used
          }
          limit={
            usageWarning.windowType === "5h"
              ? usageWarning.data.rolling_5h.limit
              : usageWarning.data.rolling_7d.limit
          }
          resetAt={
            usageWarning.windowType === "5h"
              ? usageWarning.data.rolling_5h.reset_at
              : usageWarning.data.rolling_7d.reset_at
          }
          onDismiss={() => setUsageWarning(null)}
          onUpgradeClick={() => router.push("/pricing")}
        />
      ) : null}
    </main>
  );
}
