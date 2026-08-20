"use client";

import Link from "next/link";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";

import { useAuth } from "../../lib/auth/context";
import { getSharedWorkspace, type SharedWorkspacePayload } from "../../lib/share/client";
import {
  createLocalShareSession,
  deriveSessionTitle,
  loadLocalShareSessions,
  saveLocalShareSessions,
  toWorkspaceSessions,
  type LocalShareSession,
} from "../../lib/share/local-sessions";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import type { UiChatMessage } from "../../hooks/chat-session/types";
import type { WorkspaceSession, WorkspaceSource } from "../../lib/workspace/model";
import {
  HISTORY_RAIL_MAX_WIDTH,
  HISTORY_RAIL_MIN_WIDTH,
  RIGHT_RAIL_MAX_WIDTH,
  RIGHT_RAIL_MIN_WIDTH,
} from "../../lib/workspace/ui-store";
import type { WorkspaceCitationRequest } from "../../lib/workspace/model";
import { AppModal } from "../ui/app-modal";
import { WorkspaceChatPane } from "../workspace/workspace-chat-pane";
import { WorkspaceCitationModal } from "../workspace/workspace-citation-modal";
import { WorkspaceHistoryPane } from "../workspace/workspace-history-pane";
import { WorkspaceSourcesPane } from "../workspace/workspace-sources-pane";
import shellStyles from "../workspace/workspace-shell.module.css";
import { ShareMoreSharesTab } from "./parts/share-more-shares-tab";
import { ShareOwnerHero } from "./parts/share-owner-hero";
import { ShareSourcesTab, isSourceOpenable, sourceStatusLabel } from "./parts/share-sources-tab";
import { ShareTabBar, type ShareTabId } from "./parts/share-tab-bar";
import styles from "./shared-workspace-surface.module.css";

function normalizeSemanticValue(value: string | null | undefined) {
  const normalized = value?.trim().toLowerCase();
  return normalized && normalized.length > 0 ? normalized : "unknown";
}

function loadErrorSemantic(error: string, shareToken: string) {
  if (!shareToken.trim()) {
    return "invalid";
  }
  const normalized = error.trim().toLowerCase();
  if (!normalized) {
    return "invalid";
  }
  if (normalized.includes("expired")) {
    return "expired";
  }
  if (normalized.includes("invalid")) {
    return "invalid";
  }
  return normalized;
}

function toWorkspaceSources(payload: SharedWorkspacePayload | null): WorkspaceSource[] {
  if (!payload) {
    return [];
  }
  const workspaceId = payload.knowledge_base.id;
  const workspaceName = payload.knowledge_base.title;
  return payload.sources.map((source) => ({
    id: source.id,
    workspace_id: workspaceId,
    workspace_name: workspaceName,
    title: source.file_name,
    file_name: source.file_name,
    status: source.status,
  }));
}

function readySourceIds(sources: WorkspaceSource[]) {
  return sources
    .filter((s) => {
      const st = normalizeSemanticValue(s.status);
      return st === "ready" || st === "completed";
    })
    .map((s) => s.id);
}

const TURNSTILE_SITE_KEY =
  typeof process !== "undefined" ? (process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY ?? "").trim() : "";

declare global {
  interface Window {
    turnstile?: {
      render: (
        el: HTMLElement,
        opts: {
          sitekey: string;
          callback: (token: string) => void;
          "expired-callback"?: () => void;
          "error-callback"?: () => void;
        },
      ) => string;
      reset: (widgetId?: string) => void;
    };
  }
}

/**
 * Public share page — workspace-like shell:
 * sessions + RAG chat + read-only sources.
 * No add sources, no notes, no web search, no plain chat mode (RAG locked).
 */
export function SharedWorkspaceSurface({ shareToken }: { shareToken: string }) {
  const { locale } = useUiPreferences();
  const auth = useAuth();
  const [payload, setPayload] = useState<SharedWorkspacePayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [viewerSourceId, setViewerSourceId] = useState<string | null>(null);
  const [activeCitation, setActiveCitation] = useState<WorkspaceCitationRequest | null>(null);
  const [activeTab, setActiveTab] = useState<ShareTabId>("chat");
  const composerInsertRef = useRef<((text: string) => boolean) | null>(null);
  const [localSessions, setLocalSessions] = useState<LocalShareSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [historyWidth, setHistoryWidth] = useState(20 * 16);
  const [rightWidth, setRightWidth] = useState(24.5 * 16);
  const [turnstileToken, setTurnstileToken] = useState("");
  const [selectedSourceIds, setSelectedSourceIds] = useState<string[]>([]);
  const selectionHydratedRef = useRef(false);
  const turnstileRef = useRef<HTMLDivElement | null>(null);
  const turnstileWidgetId = useRef<string | null>(null);
  const activeResizeCleanupRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadSharedWorkspace() {
      if (!shareToken.trim()) {
        setLoadError("invalid");
        setLoading(false);
        return;
      }

      setLoading(true);
      setLoadError("");

      try {
        const response = await getSharedWorkspace(shareToken);
        if (!cancelled) {
          setPayload(response);
          const stored = loadLocalShareSessions(shareToken);
          setLocalSessions(stored);
          setActiveSessionId(stored[0]?.id ?? null);
        }
      } catch (loadFailure) {
        if (!cancelled) {
          setLoadError(loadFailure instanceof Error ? loadFailure.message : "invalid");
          setPayload(null);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadSharedWorkspace();
    return () => {
      cancelled = true;
    };
  }, [shareToken]);

  // Turnstile for anonymous visitors
  useEffect(() => {
    if (!TURNSTILE_SITE_KEY || !payload || auth.token) {
      return;
    }
    let cancelled = false;
    const mount = () => {
      if (cancelled || !turnstileRef.current || !window.turnstile) {
        return;
      }
      if (turnstileWidgetId.current) {
        return;
      }
      turnstileWidgetId.current = window.turnstile.render(turnstileRef.current, {
        sitekey: TURNSTILE_SITE_KEY,
        callback: (token) => setTurnstileToken(token),
        "expired-callback": () => setTurnstileToken(""),
        "error-callback": () => setTurnstileToken(""),
      });
    };
    if (window.turnstile) {
      mount();
      return () => {
        cancelled = true;
      };
    }
    const existing = document.querySelector<HTMLScriptElement>(
      'script[src*="challenges.cloudflare.com/turnstile"]',
    );
    if (existing) {
      existing.addEventListener("load", mount);
      return () => {
        cancelled = true;
        existing.removeEventListener("load", mount);
      };
    }
    const script = document.createElement("script");
    script.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
    script.async = true;
    script.onload = mount;
    document.head.appendChild(script);
    return () => {
      cancelled = true;
    };
  }, [payload, auth.token]);

  useEffect(() => {
    return () => {
      activeResizeCleanupRef.current?.();
    };
  }, []);

  const sources = useMemo(() => toWorkspaceSources(payload), [payload]);
  const workspaceId = payload?.knowledge_base.id ?? "";

  useEffect(() => {
    selectionHydratedRef.current = false;
    setSelectedSourceIds([]);
  }, [shareToken]);

  useEffect(() => {
    if (!selectionHydratedRef.current) {
      if (sources.length === 0) {
        return;
      }
      selectionHydratedRef.current = true;
      setSelectedSourceIds(readySourceIds(sources));
      return;
    }

    const knownIds = new Set(sources.map((source) => source.id));
    setSelectedSourceIds((current) => {
      const next = current.filter((id) => knownIds.has(id));
      return next.length === current.length ? current : next;
    });
  }, [sources]);

  const handleSelectAllSources = useCallback(() => {
    setSelectedSourceIds((current) =>
      current.length === sources.length && sources.length > 0
        ? []
        : sources.map((source) => source.id),
    );
  }, [sources]);

  const handleSelectedSourceToggle = useCallback((sourceId: string) => {
    setSelectedSourceIds((current) =>
      current.includes(sourceId)
        ? current.filter((id) => id !== sourceId)
        : [...current, sourceId],
    );
  }, []);
  const sessionsAsWorkspace: WorkspaceSession[] = useMemo(
    () => toWorkspaceSessions(localSessions, workspaceId || "shared"),
    [localSessions, workspaceId],
  );

  const activeLocal = localSessions.find((s) => s.id === activeSessionId) ?? null;
  const initialMessages = activeLocal?.messages ?? null;

  const persistSessions = useCallback(
    (next: LocalShareSession[]) => {
      setLocalSessions(next);
      if (shareToken.trim()) {
        saveLocalShareSessions(shareToken, next);
      }
    },
    [shareToken],
  );

  const handleTranscriptChange = useCallback(
    (messages: UiChatMessage[]) => {
      const sid =
        messages.find((m) => m.sessionId)?.sessionId ?? activeSessionId;
      if (!sid) {
        return;
      }
      setLocalSessions((current) => {
        const exists = current.some((s) => s.id === sid);
        const next = exists
          ? current.map((session) => {
              if (session.id !== sid) {
                return session;
              }
              return {
                ...session,
                messages,
                title: session.title ?? deriveSessionTitle(messages),
                updated_at: new Date().toISOString(),
              };
            })
          : [
              {
                id: sid,
                title: deriveSessionTitle(messages),
                pinned: false,
                created_at: new Date().toISOString(),
                updated_at: new Date().toISOString(),
                messages,
              },
              ...current,
            ];
        if (shareToken.trim()) {
          saveLocalShareSessions(shareToken, next);
        }
        return next;
      });
      setActiveSessionId((current) => current ?? sid);
    },
    [activeSessionId, shareToken],
  );

  const handleSessionChange = useCallback(
    (sessionId: string | null) => {
      setActiveSessionId(sessionId);
      if (!sessionId) {
        return;
      }
      // Ensure session exists in local store when stream allocates a new id.
      if (!localSessions.some((s) => s.id === sessionId)) {
        const created: LocalShareSession = {
          id: sessionId,
          title: null,
          pinned: false,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
          messages: [],
        };
        persistSessions([created, ...localSessions]);
      }
    },
    [localSessions, persistSessions],
  );

  function startNewThread() {
    const created = createLocalShareSession();
    persistSessions([created, ...localSessions]);
    setActiveSessionId(created.id);
  }

  function selectSession(sessionId: string) {
    setActiveSessionId(sessionId);
  }

  async function handleDeleteSession(session: WorkspaceSession): Promise<boolean> {
    const next = localSessions.filter((s) => s.id !== session.id);
    persistSessions(next);
    if (activeSessionId === session.id) {
      setActiveSessionId(next[0]?.id ?? null);
    }
    return true;
  }

  function handleTogglePin(session: WorkspaceSession) {
    persistSessions(
      localSessions.map((s) =>
        s.id === session.id ? { ...s, pinned: !s.pinned, updated_at: new Date().toISOString() } : s,
      ),
    );
  }

  function handleRename(session: WorkspaceSession) {
    const nextTitle =
      typeof window !== "undefined"
        ? window.prompt(
            formatUiMessage(locale, "workspaceRenameSessionDialogLabel"),
            session.title ?? "",
          )
        : null;
    if (nextTitle == null) {
      return;
    }
    const trimmed = nextTitle.trim();
    persistSessions(
      localSessions.map((s) =>
        s.id === session.id
          ? { ...s, title: trimmed || null, updated_at: new Date().toISOString() }
          : s,
      ),
    );
  }

  function beginResize(side: "history" | "right", startX: number) {
    if (activeResizeCleanupRef.current) {
      return;
    }
    const startWidth = side === "history" ? historyWidth : rightWidth;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    function onMove(event: MouseEvent) {
      const delta = event.clientX - startX;
      if (side === "history") {
        setHistoryWidth(
          Math.min(HISTORY_RAIL_MAX_WIDTH, Math.max(HISTORY_RAIL_MIN_WIDTH, startWidth + delta)),
        );
      } else {
        setRightWidth(
          Math.min(RIGHT_RAIL_MAX_WIDTH, Math.max(RIGHT_RAIL_MIN_WIDTH, startWidth - delta)),
        );
      }
    }

    function finish() {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", finish);
      activeResizeCleanupRef.current = null;
    }

    activeResizeCleanupRef.current = finish;
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", finish);
  }

  const bodyStyle = {
    "--workspace-history-rail-width": `${historyWidth}px`,
    "--workspace-right-rail-width": `${rightWidth}px`,
  } as CSSProperties;

  const registerComposerInsert = useCallback(
    (handler: ((text: string) => boolean) | null) => {
      composerInsertRef.current = handler;
    },
    [],
  );

  /** Source-detail CTA: prefill a question and jump back to the chat tab. */
  function handleAskAboutSource(source: WorkspaceSource) {
    const text = formatUiMessage(locale, "sharedPublic.sourceAskTemplate", {
      name: source.file_name,
    });
    setViewerSourceId(null);
    setActiveTab("chat");
    // Chat pane stays mounted across tabs; insert on the next frame.
    requestAnimationFrame(() => {
      composerInsertRef.current?.(text);
    });
  }

  const owner = payload?.owner ?? null;
  // Owner profile entry points (hero link, "more shares" tab) require the
  // owner's opt-in; an explicit profile_enabled=false hides them.
  const ownerProfileVisible =
    Boolean(owner?.user_id?.trim()) && owner?.profile_enabled !== false;
  const title =
    payload?.knowledge_base.title?.trim() ||
    formatUiMessage(locale, "sharedPublic.pageTitle");
  const description = payload?.knowledge_base.description?.trim() || "";
  const viewerSource = sources.find((s) => s.id === viewerSourceId) ?? null;

  const needsTurnstile = Boolean(TURNSTILE_SITE_KEY && !auth.token);
  const turnstileReady = !needsTurnstile || Boolean(turnstileToken.trim());

  if (loading) {
    return (
      <main className={shellStyles.shell} data-testid="shared-workspace-shell">
        <header className={shellStyles.topBar}>
          <div className={shellStyles.topBarBrand}>
            <Link className="app-link app-link-muted" href="/">
              {formatUiMessage(locale, "sharedPublic.backHomeAction")}
            </Link>
          </div>
        </header>
        <section className={styles.centerPane} role="status">
          <p className={styles.flushText}>{formatUiMessage(locale, "sharedPublic.loading")}</p>
        </section>
      </main>
    );
  }

  if (loadError || !payload) {
    return (
      <main className={shellStyles.shell} data-testid="shared-workspace-shell">
        <header className={shellStyles.topBar}>
          <div className={shellStyles.topBarBrand}>
            <Link className="app-link app-link-muted" href="/">
              {formatUiMessage(locale, "sharedPublic.backHomeAction")}
            </Link>
          </div>
        </header>
        <section className={styles.centerPane}>
          <div className={`app-surface-card ${styles.errorCard}`}>
            <h2 className={`app-page-title ${styles.errorTitle}`}>
              {formatUiMessage(locale, "sharedPublic.invalidLinkTitle")}
            </h2>
            <p className="app-page-subtitle">
              {formatUiMessage(locale, "sharedPublic.invalidLinkBody")}
            </p>
            <code className={styles.semanticCode}>
              {loadErrorSemantic(loadError, shareToken)}
            </code>
          </div>
        </section>
      </main>
    );
  }

  const historyPane = (
    <WorkspaceHistoryPane
      activeSessionId={activeSessionId}
      onDeleteSession={handleDeleteSession}
      onNewThread={startNewThread}
      onRenameSession={handleRename}
      onSelectSession={selectSession}
      onTogglePinSession={handleTogglePin}
      sessions={sessionsAsWorkspace}
      workspaceId={workspaceId}
    />
  );

  return (
    <main className={shellStyles.shell} data-testid="shared-workspace-shell">
      <ShareOwnerHero
        allowDownload={payload.share.allow_download}
        owner={owner}
        sourceCount={payload.sources.length}
        workspaceDescription={description}
        workspaceTitle={title}
      />

      <ShareTabBar
        activeTab={activeTab}
        showShares={ownerProfileVisible}
        sourceCount={sources.length}
        onChange={setActiveTab}
      />

      <div
        className={shellStyles.body}
        style={{ ...bodyStyle, display: activeTab === "chat" ? undefined : "none" }}
      >
        <aside className={shellStyles.desktopHistoryRail} data-testid="shared-history-rail">
          {historyPane}
        </aside>

        <div
          aria-orientation="vertical"
          className={shellStyles.desktopRailResizer}
          onMouseDown={(event) => {
            event.preventDefault();
            beginResize("history", event.clientX);
          }}
          role="separator"
        />

        <section className={shellStyles.panePanel} data-testid="shared-chat-pane">
          {needsTurnstile ? (
            <div className={styles.turnstileHost} ref={turnstileRef} data-testid="share-turnstile" />
          ) : null}

          {!turnstileReady ? (
            <p className={styles.mutedText} role="status">
              {formatUiMessage(locale, "sharedPublic.turnstileRequired")}
            </p>
          ) : null}

          <WorkspaceChatPane
            initialMessages={initialMessages}
            lockedCapabilities={["rag"]}
            onSelectCitation={setActiveCitation}
            onSessionChange={handleSessionChange}
            onTranscriptChange={handleTranscriptChange}
            registerComposerInsert={registerComposerInsert}
            selectedSourceIds={selectedSourceIds}
            sessionId={activeSessionId}
            shareToken={shareToken}
            turnstileToken={turnstileToken || null}
            workspaceId={workspaceId}
          />
        </section>

        <div
          aria-orientation="vertical"
          className={shellStyles.desktopRailResizer}
          onMouseDown={(event) => {
            event.preventDefault();
            beginResize("right", event.clientX);
          }}
          role="separator"
        />

        <aside
          className={shellStyles.desktopRightRail}
          aria-label={formatUiMessage(locale, "workspaceRightRail.label")}
          data-testid="shared-desktop-right-rail"
        >
          <WorkspaceSourcesPane
            activeViewerSourceId={viewerSourceId}
            error=""
            focusedSourceId={viewerSourceId}
            loading={false}
            onAddUrlSource={() => false}
            onDeleteSource={() => undefined}
            onOpenSource={(sourceId) => setViewerSourceId(sourceId)}
            onReindexSource={() => undefined}
            onSelectAll={handleSelectAllSources}
            onSelectedSourceToggle={handleSelectedSourceToggle}
            onUploadFiles={() => undefined}
            onUrlSourceChange={() => undefined}
            polling={false}
            readOnly
            allowSelection
            selectedSourceIds={selectedSourceIds}
            sources={sources}
            urlSource=""
          />
        </aside>
      </div>

      {activeTab === "sources" ? (
        <ShareSourcesTab sources={sources} onOpenSource={(sourceId) => setViewerSourceId(sourceId)} />
      ) : null}

      {activeTab === "shares" && ownerProfileVisible && owner?.user_id?.trim() ? (
        <ShareMoreSharesTab currentShareToken={shareToken} userId={owner.user_id.trim()} />
      ) : null}

      <AppModal
        open={Boolean(viewerSource)}
        size="lg"
        title={
          viewerSource?.file_name ??
          formatUiMessage(locale, "sharedPublic.sourceDetailTitle")
        }
        closeLabel={formatUiMessage(locale, "workspaceRightRail.closeViewerAction")}
        testId="shared-source-detail-modal"
        onClose={() => setViewerSourceId(null)}
        footer={
          viewerSource && isSourceOpenable(viewerSource.status) ? (
            <button
              type="button"
              className="app-button-primary"
              data-testid="shared-source-ask-action"
              onClick={() => handleAskAboutSource(viewerSource)}
            >
              {formatUiMessage(locale, "sharedPublic.sourceAskAction")}
            </button>
          ) : undefined
        }
      >
        {viewerSource ? (
          <div className={styles.sourceDetail}>
            <div className={styles.metaPair}>
              <span className={styles.metaLabel}>
                {formatUiMessage(locale, "sharedPublic.sourceDetailStatus")}
              </span>
              <code
                className={styles.semanticCode}
                data-status={normalizeSemanticValue(viewerSource.status)}
              >
                {sourceStatusLabel(locale, viewerSource.status)}
              </code>
            </div>
            <p className={styles.mutedText}>
              {formatUiMessage(locale, "sharedPublic.sourceDetailBody")}
            </p>
          </div>
        ) : null}
      </AppModal>
      <WorkspaceCitationModal
        citationRequest={activeCitation}
        onClose={() => setActiveCitation(null)}
        onOpenSource={(sourceId) => {
          setViewerSourceId(sourceId);
          setActiveCitation(null);
        }}
        workspaceId={workspaceId}
      />
    </main>
  );
}
