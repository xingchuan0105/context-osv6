"use client";

import Link from "next/link";
import { useMemo } from "react";

import type { ConversationSummary } from "../../lib/chat/client";
import { conversationHref } from "../../lib/chat/session-url";
import type { DashboardWorkspace } from "../../lib/dashboard/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import { appNavHref } from "../../lib/navigation/nav-config";
import { desktopAppHref } from "../../lib/runtime/desktop-app-href";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./chat-shell.module.css";

const RECENT_SESSION_LIMIT = 30;
const WORKSPACE_LIMIT = 8;

type GlobalConversationRailProps = {
  activeSessionId: string | null;
  error: boolean;
  loading: boolean;
  mobileOpen: boolean;
  navigationDisabled: boolean;
  sessions: readonly ConversationSummary[];
  workspaces: readonly DashboardWorkspace[];
  onNewChat: () => void;
  onRetry: () => void;
  onSelectPersonal: (sessionId: string) => void;
};

function workspaceTitle(workspace: DashboardWorkspace): string {
  return (workspace.title || workspace.name || workspace.workspace_id).trim();
}

export function GlobalConversationRail({
  activeSessionId,
  error,
  loading,
  mobileOpen,
  navigationDisabled,
  sessions,
  workspaces,
  onNewChat,
  onRetry,
  onSelectPersonal,
}: GlobalConversationRailProps) {
  const { locale } = useUiPreferences();
  const workspaceNames = useMemo(
    () =>
      new Map(
        workspaces.map((workspace) => [
          workspace.workspace_id,
          workspaceTitle(workspace),
        ]),
      ),
    [workspaces],
  );
  const recentSessions = sessions.slice(0, RECENT_SESSION_LIMIT);
  const untitled = formatUiMessage(locale, "chat.untitled");

  return (
    <aside
      aria-label={formatUiMessage(locale, "chat.sidebarLabel")}
      className={`${styles.sidebar}${mobileOpen ? ` ${styles.sidebarMobileOpen}` : ""}`}
      id="global-conversation-rail"
    >
      <Link
        aria-disabled={navigationDisabled || undefined}
        className={`${styles.newChat}${navigationDisabled ? ` ${styles.navigationDisabled}` : ""}`}
        data-testid="chat-new-conversation"
        href={appNavHref("chat")}
        onClick={(event) => {
          if (navigationDisabled) {
            event.preventDefault();
            return;
          }
          onNewChat();
        }}
        tabIndex={navigationDisabled ? -1 : undefined}
      >
        <span aria-hidden="true">＋</span>
        {formatUiMessage(locale, "chat.newConversation")}
      </Link>

      <section className={styles.railSection}>
        <h2 className={styles.sectionTitle}>
          {formatUiMessage(locale, "chat.recent")}
        </h2>
        {loading ? (
          <p className={styles.muted}>{formatUiMessage(locale, "chat.loading")}</p>
        ) : error ? (
          <div className={styles.errorBlock} role="status">
            <p className={styles.muted}>
              {formatUiMessage(locale, "chat.listLoadError")}
            </p>
            <button className={styles.retryButton} type="button" onClick={onRetry}>
              {formatUiMessage(locale, "chat.retry")}
            </button>
          </div>
        ) : recentSessions.length === 0 ? (
          <p className={styles.muted}>{formatUiMessage(locale, "chat.noRecent")}</p>
        ) : (
          <nav aria-label={formatUiMessage(locale, "chat.recent")}>
            <ul className={styles.list}>
              {recentSessions.map((session) => {
                const workspaceId =
                  session.scope_kind === "workspace"
                    ? session.workspace_id?.trim() || null
                    : null;
                const selected =
                  session.scope_kind === "personal" && session.id === activeSessionId;
                const contextLabel = session.scope_kind === "workspace"
                  ? formatUiMessage(locale, "chat.workspaceContext", {
                      name:
                        session.workspace_name?.trim() ||
                        (workspaceId ? workspaceNames.get(workspaceId) ?? workspaceId : untitled),
                    })
                  : formatUiMessage(locale, "chat.personalContext");

                return (
                  <li key={session.id}>
                    <Link
                      aria-disabled={navigationDisabled || undefined}
                      aria-current={selected ? "page" : undefined}
                      className={`${styles.sessionLink}${
                        selected ? ` ${styles.sessionLinkActive}` : ""
                      }${navigationDisabled ? ` ${styles.navigationDisabled}` : ""}`}
                      href={desktopAppHref(conversationHref(session))}
                      onClick={(event) => {
                        if (navigationDisabled) {
                          event.preventDefault();
                          return;
                        }
                        if (session.scope_kind === "personal") {
                          onSelectPersonal(session.id);
                        }
                      }}
                      tabIndex={navigationDisabled ? -1 : undefined}
                    >
                      <span className={styles.sessionTitle}>
                        {session.title?.trim() || untitled}
                      </span>
                      <span className={styles.sessionContext}>{contextLabel}</span>
                    </Link>
                  </li>
                );
              })}
            </ul>
          </nav>
        )}
      </section>

      <section className={styles.railSection}>
        <div className={styles.sectionHeadingRow}>
          <h2 className={styles.sectionTitle}>
            {formatUiMessage(locale, "chat.workspaces")}
          </h2>
          <Link
            aria-disabled={navigationDisabled || undefined}
            className={`${styles.sectionLink}${navigationDisabled ? ` ${styles.navigationDisabled}` : ""}`}
            href={appNavHref("dashboard")}
            onClick={(event) => {
              if (navigationDisabled) {
                event.preventDefault();
              }
            }}
            tabIndex={navigationDisabled ? -1 : undefined}
          >
            {formatUiMessage(locale, "chat.allWorkspaces")}
          </Link>
        </div>
        <ul className={styles.list}>
          {workspaces.slice(0, WORKSPACE_LIMIT).map((workspace) => (
            <li key={workspace.workspace_id}>
              <Link
                aria-disabled={navigationDisabled || undefined}
                className={`${styles.workspaceLink}${navigationDisabled ? ` ${styles.navigationDisabled}` : ""}`}
                href={desktopAppHref(`/dashboard/${workspace.workspace_id}`)}
                onClick={(event) => {
                  if (navigationDisabled) {
                    event.preventDefault();
                  }
                }}
                tabIndex={navigationDisabled ? -1 : undefined}
              >
                {workspaceTitle(workspace)}
              </Link>
            </li>
          ))}
        </ul>
      </section>
    </aside>
  );
}
