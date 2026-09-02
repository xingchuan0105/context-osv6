"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { useAuth } from "../../lib/auth/context";
import {
  getChatSession,
  listChatSessions,
  sortConversationsByRecent,
  type ConversationSummary,
} from "../../lib/chat/client";
import { conversationHref } from "../../lib/chat/session-url";
import {
  listWorkspaces,
  type DashboardWorkspace,
} from "../../lib/dashboard/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import { appNavHref } from "../../lib/navigation/nav-config";
import {
  desktopAppHref,
  resolveChatSessionIdFromRoute,
} from "../../lib/runtime/desktop-app-href";
import { useUiPreferences } from "../../lib/ui-preferences";
import type {
  WorkspaceCapability,
} from "../../lib/workspace/capabilities";
import type { WorkspaceWebSourcesRequest } from "../../lib/workspace/model";
import { AppTopBar } from "../app-top-bar";
import { ChatCanvas } from "./chat-canvas";
import { GlobalConversationRail } from "./global-conversation-rail";
import { WorkspaceWebSourcesModal } from "../workspace/workspace-web-sources-modal";
import styles from "./chat-shell.module.css";

const PERSONAL_CHAT_CAPABILITIES: WorkspaceCapability[] = ["search"];

type SessionResolution =
  | { status: "ready"; sessionId: string | null }
  | { status: "loading" | "redirecting" | "error"; sessionId: string };

export function ChatShell({ children }: { children?: ReactNode }) {
  const auth = useAuth();
  const router = useRouter();
  const pathname = usePathname() ?? "/chat";
  const searchParams = useSearchParams();
  const { locale } = useUiPreferences();
  const routeSessionId = resolveChatSessionIdFromRoute(
    pathname,
    searchParams.get("session"),
  );
  const trustedPersonalSessionIdRef = useRef<string | null>(null);
  const previousRouteSessionIdRef = useRef<string | null>(routeSessionId);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(routeSessionId);
  const [sessionResolution, setSessionResolution] = useState<SessionResolution>(() =>
    routeSessionId
      ? { status: "loading", sessionId: routeSessionId }
      : { status: "ready", sessionId: null },
  );
  const [sessions, setSessions] = useState<ConversationSummary[]>([]);
  const [workspaces, setWorkspaces] = useState<DashboardWorkspace[]>([]);
  const [loading, setLoading] = useState(true);
  const [listError, setListError] = useState(false);
  const [railOpen, setRailOpen] = useState(false);
  const [isStreaming, setIsStreaming] = useState(false);
  const [resetEpoch, setResetEpoch] = useState(0);
  const [exactLookupKey, setExactLookupKey] = useState(0);
  const [refreshKey, setRefreshKey] = useState(0);
  const [activeWebSources, setActiveWebSources] =
    useState<WorkspaceWebSourcesRequest | null>(null);

  useEffect(() => {
    const previousRouteSessionId = previousRouteSessionIdRef.current;
    previousRouteSessionIdRef.current = routeSessionId;
    setActiveSessionId(routeSessionId);
    setRailOpen(false);

    if (!routeSessionId) {
      setSessionResolution({ status: "ready", sessionId: null });
      if (previousRouteSessionId !== null) {
        trustedPersonalSessionIdRef.current = null;
        setResetEpoch((current) => current + 1);
      }
      return;
    }

    setSessionResolution(
      trustedPersonalSessionIdRef.current === routeSessionId
        ? { status: "ready", sessionId: routeSessionId }
        : { status: "loading", sessionId: routeSessionId },
    );
  }, [routeSessionId]);

  useEffect(() => {
    if (
      !routeSessionId ||
      !auth.token ||
      trustedPersonalSessionIdRef.current === routeSessionId
    ) {
      return;
    }

    let cancelled = false;
    setSessionResolution({ status: "loading", sessionId: routeSessionId });

    void getChatSession(auth.token, routeSessionId)
      .then((session) => {
        if (cancelled) {
          return;
        }
        if (session.id !== routeSessionId) {
          setSessionResolution({ status: "error", sessionId: routeSessionId });
          return;
        }
        if (session.scope_kind === "workspace") {
          if (!session.workspace_id?.trim()) {
            setSessionResolution({ status: "error", sessionId: routeSessionId });
            return;
          }
          setSessionResolution({ status: "redirecting", sessionId: routeSessionId });
          router.replace(desktopAppHref(conversationHref(session)));
          return;
        }
        if (session.scope_kind !== "personal" || session.workspace_id?.trim()) {
          setSessionResolution({ status: "error", sessionId: routeSessionId });
          return;
        }
        setActiveSessionId(routeSessionId);
        setSessionResolution({ status: "ready", sessionId: routeSessionId });
      })
      .catch(() => {
        if (!cancelled) {
          setSessionResolution({ status: "error", sessionId: routeSessionId });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [auth.token, exactLookupKey, routeSessionId, router]);

  useEffect(() => {
    if (!auth.token) {
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setListError(false);

    void Promise.allSettled([
      listChatSessions(auth.token),
      listWorkspaces(auth.token),
    ]).then(([sessionResult, workspaceResult]) => {
      if (cancelled) {
        return;
      }

      if (sessionResult.status === "fulfilled") {
        setSessions(sortConversationsByRecent(sessionResult.value.sessions));
      } else {
        setSessions([]);
        setListError(true);
      }

      if (workspaceResult.status === "fulfilled") {
        setWorkspaces(workspaceResult.value.workspaces);
      } else {
        setWorkspaces([]);
      }

      setLoading(false);
    });

    return () => {
      cancelled = true;
    };
  }, [auth.token, refreshKey]);

  const syncPersonalSessionUrl = useCallback(
    (sessionId: string | null) => {
      trustedPersonalSessionIdRef.current = sessionId;
      setActiveSessionId(sessionId);
      setSessionResolution({ status: "ready", sessionId });
      const href = sessionId
        ? conversationHref({ id: sessionId, workspace_id: null })
        : appNavHref("chat");
      router.replace(desktopAppHref(href));
    },
    [router],
  );

  const startNewConversation = useCallback(() => {
    trustedPersonalSessionIdRef.current = null;
    setActiveSessionId(null);
    setSessionResolution({ status: "ready", sessionId: null });
    setActiveWebSources(null);
    setRailOpen(false);
    setResetEpoch((current) => current + 1);
  }, []);

  const canvasReady =
    sessionResolution.status === "ready" &&
    sessionResolution.sessionId === activeSessionId;

  return (
    <main className={styles.shell} data-testid="chat-shell">
      <AppTopBar locale={locale} />

      <div className={styles.body}>
        <GlobalConversationRail
          activeSessionId={activeSessionId}
          error={listError}
          loading={loading}
          mobileOpen={railOpen}
          navigationDisabled={isStreaming}
          onNewChat={startNewConversation}
          onRetry={() => setRefreshKey((current) => current + 1)}
          onSelectPersonal={(sessionId) => {
            trustedPersonalSessionIdRef.current = null;
            setActiveSessionId(sessionId);
            setSessionResolution({ status: "loading", sessionId });
            setActiveWebSources(null);
            setRailOpen(false);
            if (sessionId === routeSessionId) {
              setExactLookupKey((current) => current + 1);
            }
          }}
          sessions={sessions}
          workspaces={workspaces}
        />

        <section className={styles.canvas}>
          <div className={styles.contextBar}>
            <button
              aria-controls="global-conversation-rail"
              aria-expanded={railOpen}
              aria-label={formatUiMessage(locale, "chat.sidebarLabel")}
              className={styles.railToggle}
              onClick={() => setRailOpen((current) => !current)}
              type="button"
            >
              ☰
            </button>
            <span className={styles.contextPill}>
              {formatUiMessage(locale, "chat.personalContext")}
            </span>
          </div>
          <div className={styles.canvasBody}>
            {canvasReady ? (
              <ChatCanvas
                availableCapabilities={PERSONAL_CHAT_CAPABILITIES}
                onOpenWebSources={setActiveWebSources}
                onSessionChange={(sessionId) => {
                  syncPersonalSessionUrl(sessionId);
                  setRefreshKey((current) => current + 1);
                }}
                onStreamingChange={setIsStreaming}
                resetEpoch={resetEpoch}
                selectedSourceIds={[]}
                sessionId={activeSessionId}
                workspaceId={null}
              />
            ) : sessionResolution.status === "error" ? (
              <div
                className={styles.sessionTerminal}
                role="alert"
              >
                <p>{formatUiMessage(locale, "chat.loadError")}</p>
                <button
                  className={styles.retryButton}
                  onClick={() => setExactLookupKey((current) => current + 1)}
                  type="button"
                >
                  {formatUiMessage(locale, "chat.retry")}
                </button>
              </div>
            ) : (
              <div className={styles.sessionTerminal} role="status">
                {formatUiMessage(locale, "chat.loading")}
              </div>
            )}
          </div>
        </section>
      </div>

      <WorkspaceWebSourcesModal
        request={activeWebSources}
        onClose={() => setActiveWebSources(null)}
      />
      {children}
    </main>
  );
}
