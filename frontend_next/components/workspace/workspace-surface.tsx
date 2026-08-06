"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type Dispatch,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type SetStateAction,
  type TouchEvent as ReactTouchEvent,
} from "react";
import { useRouter, useSearchParams } from "next/navigation";

import { useWorkspaceData } from "../../hooks/use-workspace-data";
import { useAuth } from "../../lib/auth/context";
import type { UsageWindowBucket, UsageWindowResponse } from "../../lib/billing/api";
import { probePricingRevampUsageWindow } from "../../lib/billing/featureFlag";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import type { WorkspaceWebSourcesRequest } from "../../lib/workspace/model";
import {
  workspaceDeepLinkHref,
  workspaceSessionHref,
} from "../../lib/workspace/session-url";
import {
  DEFAULT_WORKSPACE_UI_STATE,
  HISTORY_RAIL_MAX_WIDTH,
  HISTORY_RAIL_MIN_WIDTH,
  RIGHT_RAIL_MAX_WIDTH,
  RIGHT_RAIL_MIN_WIDTH,
  useWorkspaceUi,
} from "../../lib/workspace/ui-store";
import styles from "./workspace-shell.module.css";
import { WorkspaceChatPane } from "./workspace-chat-pane";
import { WorkspaceCitationModal } from "./workspace-citation-modal";
import { WorkspaceHistoryPane } from "./workspace-history-pane";
import { WorkspaceRightRail } from "./workspace-right-rail";
import { WorkspaceTopBar } from "./workspace-top-bar";
import { WorkspaceWebSourcesModal } from "./workspace-web-sources-modal";
import { UsageWarningToast } from "../billing/UsageWarningToast";
import { ReferralInviteSurface } from "../referral/referral-invite-surface";

function usageWindowPressure(bucket: UsageWindowBucket): number {
  if (bucket.limit <= 0) {
    return 0;
  }
  return bucket.used / bucket.limit;
}

const USAGE_POLL_INTERVAL_MS = 5 * 60 * 1000;

function pickLimitWindowType(
  hits: { rolling_5h: boolean; rolling_7d: boolean },
  rolling5h: UsageWindowBucket,
  rolling7d: UsageWindowBucket,
): "5h" | "7d" {
  if (hits.rolling_5h && hits.rolling_7d) {
    return usageWindowPressure(rolling5h) >= usageWindowPressure(rolling7d) ? "5h" : "7d";
  }
  return hits.rolling_5h ? "5h" : "7d";
}

function applyUsageWindowLimits(
  usageWindow: UsageWindowResponse,
  router: ReturnType<typeof useRouter>,
  setUsageWarning: Dispatch<
    SetStateAction<{
      threshold: 80 | 95;
      percentage: number;
      windowType: "5h" | "7d";
      data: UsageWindowResponse;
    } | null>
  >,
) {
  if (usageWindow.hard_limit_hit.rolling_5h || usageWindow.hard_limit_hit.rolling_7d) {
    const reason = pickLimitWindowType(
      usageWindow.hard_limit_hit,
      usageWindow.rolling_5h,
      usageWindow.rolling_7d,
    );
    router.push(`/upgrade/paywall?reason=${reason}`);
    return;
  }

  if (usageWindow.soft_limit_hit.rolling_5h || usageWindow.soft_limit_hit.rolling_7d) {
    const windowType = pickLimitWindowType(
      usageWindow.soft_limit_hit,
      usageWindow.rolling_5h,
      usageWindow.rolling_7d,
    );
    const bucket = windowType === "5h" ? usageWindow.rolling_5h : usageWindow.rolling_7d;
    setUsageWarning({
      threshold: bucket.percentage >= 95 ? 95 : 80,
      percentage: bucket.percentage,
      windowType,
      data: usageWindow,
    });
    return;
  }

  setUsageWarning(null);
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

export function WorkspaceSurface({ workspaceId }: { workspaceId: string }) {
  const auth = useAuth();
  const router = useRouter();
  const searchParams = useSearchParams();
  const sessionFromUrl = searchParams.get("session");
  const sourceFromUrl = searchParams.get("source")?.trim() || null;
  const { locale } = useUiPreferences();
  const isMobile = useIsMobile();
  const workspaceUi = useWorkspaceUi(workspaceId);
  const appliedSourceDeepLinkRef = useRef<string | null>(null);
  const {
    workspace, workspaceTitleDraft, setWorkspaceTitleDraft,
    sessions, activeSessionId, setActiveSessionId, workspaceLoadError,
    renameSessionTarget, renameSessionTitle, setRenameSessionTitle,
    reloadSessions, saveWorkspaceTitle, createWorkspaceFlow, startNewThread: rawStartNewThread,
    toggleSessionPin, renameSession, dismissRename, submitRenameSession, removeSession,
    renameSubmitting,
  } = useWorkspaceData(workspaceId, { preferredSessionId: sessionFromUrl });
  const [renameSessionError, setRenameSessionError] = useState("");

  function handleDismissRename() {
    dismissRename();
    setRenameSessionError("");
  }

  const syncSessionUrl = useCallback(
    (sessionId: string | null) => {
      // Always replace: comparing against a stale searchParams closure is
      // easy to get wrong after programmatic query updates; replace is cheap.
      router.replace(workspaceSessionHref(workspaceId, sessionId));
    },
    [router, workspaceId],
  );

  function selectSession(sessionId: string) {
    setActiveSessionId(sessionId);
    syncSessionUrl(sessionId);
  }

  function startNewThread() {
    rawStartNewThread();
    syncSessionUrl(null);
    workspaceUi.setActiveCitation(null);
    workspaceUi.setFocusedSourceId(null);
  }
  const [activeWebSources, setActiveWebSources] =
    useState<WorkspaceWebSourcesRequest | null>(null);
  const [openViewerSourceId, setOpenViewerSourceId] = useState<string | null>(null);
  const [usageWarning, setUsageWarning] = useState<{
    threshold: 80 | 95;
    percentage: number;
    windowType: "5h" | "7d";
    data: UsageWindowResponse;
  } | null>(null);
  const activeResizeCleanupRef = useRef<(() => void) | null>(null);
  const recheckUsageRef = useRef<(() => void) | null>(null);
  const composerInsertRef = useRef<((text: string) => boolean) | null>(null);

  const handleInsertIntoComposer = useCallback((text: string): boolean => {
    return composerInsertRef.current?.(text) ?? false;
  }, []);

  const registerComposerInsert = useCallback((handler: ((text: string) => boolean) | null) => {
    composerInsertRef.current = handler;
  }, []);

  // Cmd+K / external `?source=` — open right rail + source viewer once, then
  // strip the one-shot query (keep `?session=` if present).
  useEffect(() => {
    if (!sourceFromUrl) {
      appliedSourceDeepLinkRef.current = null;
      return;
    }
    if (appliedSourceDeepLinkRef.current === sourceFromUrl) {
      return;
    }
    appliedSourceDeepLinkRef.current = sourceFromUrl;
    workspaceUi.setFocusedSourceId(sourceFromUrl);
    workspaceUi.setRightRailOpen(true);
    setOpenViewerSourceId(sourceFromUrl);
  }, [sourceFromUrl, workspaceUi]);

  const handleOpenSourceConsumed = useCallback(() => {
    setOpenViewerSourceId(null);
    if (!sourceFromUrl) {
      return;
    }
    router.replace(
      workspaceDeepLinkHref(workspaceId, {
        sessionId: sessionFromUrl,
        sourceId: null,
      }),
    );
  }, [router, sessionFromUrl, sourceFromUrl, workspaceId]);

  useEffect(() => {
    if (!auth.initialized || !auth.token || !auth.user?.id) {
      return;
    }

    let cancelled = false;
    let intervalId: ReturnType<typeof setInterval> | undefined;

    async function checkUsage() {
      const probe = await probePricingRevampUsageWindow();
      if (cancelled || !probe.enabled || !probe.usageWindow) {
        return;
      }

      applyUsageWindowLimits(probe.usageWindow, router, setUsageWarning);
    }

    recheckUsageRef.current = () => {
      void checkUsage();
    };

    void checkUsage();
    intervalId = setInterval(() => {
      void checkUsage();
    }, USAGE_POLL_INTERVAL_MS);

    return () => {
      cancelled = true;
      recheckUsageRef.current = null;
      if (intervalId) {
        clearInterval(intervalId);
      }
    };
  }, [auth.initialized, auth.token, auth.user?.id, router]);

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

  const historyPane = (
    <WorkspaceHistoryPane
      activeSessionId={activeSessionId}
      onDeleteSession={(session) => removeSession(session)}
      onNewThread={() => void startNewThread()}
      onRenameSession={(session) => {
        setRenameSessionError("");
        renameSession(session);
      }}
      onRequestClose={() => workspaceUi.setHistoryRailOpen(false)}
      onSelectSession={selectSession}
      onTogglePinSession={(session) => void toggleSessionPin(session)}
      sessions={sessions}
      workspaceId={workspaceId}
    />
  );

  const rightRail = (
    <WorkspaceRightRail
      focusedSourceId={workspaceUi.focusedSourceId}
      openSourceId={openViewerSourceId}
      onOpenSourceConsumed={handleOpenSourceConsumed}
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

  function handleRailResizeKeyDown(side: "history" | "right", event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
      return;
    }

    event.preventDefault();
    const step = event.key === "ArrowRight" ? 16 : -16;

    if (side === "history") {
      workspaceUi.setHistoryRailWidth(workspaceUi.historyRailWidth + step);
      return;
    }

    workspaceUi.setRightRailWidth(workspaceUi.rightRailWidth - step);
  }

  const bodyStyle = {
    "--workspace-history-rail-width": `${workspaceUi.historyRailWidth}px`,
    "--workspace-right-rail-width": `${workspaceUi.rightRailWidth}px`,
  } as CSSProperties;

  const workspaceUnavailableTitle = formatUiMessage(locale, "workspaceUnavailable");
  const workspaceUnavailableHint = formatUiMessage(locale, "workspaceUnavailableBody");

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
                {formatUiMessage(locale, "dashboardNewWorkspace")}
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
              aria-label={formatUiMessage(locale, "workspaceResizeHistoryRailLabel")}
              aria-orientation="vertical"
              aria-valuemax={HISTORY_RAIL_MAX_WIDTH}
              aria-valuemin={HISTORY_RAIL_MIN_WIDTH}
              aria-valuenow={Math.round(workspaceUi.historyRailWidth)}
              className={styles.desktopRailResizer}
              onKeyDown={(event) => handleRailResizeKeyDown("history", event)}
              onMouseDown={(event) => startDesktopMouseResize("history", event)}
              onPointerDown={(event) => startDesktopPointerResize("history", event)}
              onTouchStart={(event) => startDesktopTouchResize("history", event)}
              role="separator"
              tabIndex={0}
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
                  workspaceUi.setActiveCitation(null);
                }}
                onSelectCitation={(request) => {
                  workspaceUi.setActiveCitation(request);
                }}
                onSessionActivity={() => {
                  recheckUsageRef.current?.();
                  // Pass current selection explicitly so a new thread (null) does not
                  // fall back to sessions[0] and wipe the live progress card.
                  void reloadSessions(activeSessionId);
                }}
                onSessionChange={(sessionId) => {
                  setActiveSessionId(sessionId);
                  syncSessionUrl(sessionId);
                  workspaceUi.setActiveCitation(null);
                  void reloadSessions(sessionId);
                }}
                registerComposerInsert={registerComposerInsert}
                selectedSourceIds={workspaceUi.selectedSourceIds}
                sessionId={activeSessionId}
                workspaceId={workspaceId}
              />
            </section>

            <div
              aria-label={formatUiMessage(locale, "workspaceResizeRightRailLabel")}
              aria-orientation="vertical"
              aria-valuemax={RIGHT_RAIL_MAX_WIDTH}
              aria-valuemin={RIGHT_RAIL_MIN_WIDTH}
              aria-valuenow={Math.round(workspaceUi.rightRailWidth)}
              className={styles.desktopRailResizer}
              onKeyDown={(event) => handleRailResizeKeyDown("right", event)}
              onMouseDown={(event) => startDesktopMouseResize("right", event)}
              onPointerDown={(event) => startDesktopPointerResize("right", event)}
              onTouchStart={(event) => startDesktopTouchResize("right", event)}
              role="separator"
              tabIndex={0}
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
            onOpenSource={(sourceId) => {
              workspaceUi.setFocusedSourceId(sourceId);
              workspaceUi.setRightRailOpen(true);
              setOpenViewerSourceId(sourceId);
            }}
            workspaceId={workspaceId}
          />

          <WorkspaceWebSourcesModal
            request={activeWebSources}
            onClose={() => setActiveWebSources(null)}
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
            handleDismissRename();
          }}
        >
          <div
            className={styles.modalCard}
            role="dialog"
            aria-modal="true"
            aria-label={formatUiMessage(locale, "workspaceRenameSessionDialogLabel")}
            onClick={(event) => event.stopPropagation()}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                handleDismissRename();
              }
            }}
          >
            <form
              className={styles.dialogForm}
              onSubmit={(event) => {
                event.preventDefault();
                if (renameSubmitting) {
                  return;
                }
                setRenameSessionError("");
                void submitRenameSession().then((ok) => {
                  if (!ok) {
                    setRenameSessionError(formatUiMessage(locale, "workspaceRenameSessionFailed"));
                  }
                });
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
              {renameSessionError ? (
                <p className="app-notice-banner" role="alert">
                  {renameSessionError}
                </p>
              ) : null}
              <div className={styles.dialogActions}>
                <button
                  className={styles.secondaryButton}
                  type="button"
                  onClick={() => {
                    handleDismissRename();
                  }}
                >
                  {formatUiMessage(locale, "commonCancel")}
                </button>
                <button className={styles.primaryButton} disabled={renameSubmitting} type="submit">
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
              ? (usageWarning.data.rolling_5h.used_tokens_approx ??
                usageWarning.data.rolling_5h.used)
              : (usageWarning.data.rolling_7d.used_tokens_approx ??
                usageWarning.data.rolling_7d.used)
          }
          limit={
            usageWarning.windowType === "5h"
              ? (usageWarning.data.rolling_5h.limit_tokens_approx ??
                usageWarning.data.rolling_5h.limit)
              : (usageWarning.data.rolling_7d.limit_tokens_approx ??
                usageWarning.data.rolling_7d.limit)
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

      <ReferralInviteSurface />
    </main>
  );
}
