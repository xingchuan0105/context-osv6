"use client";

import { Fragment, type FormEvent, useEffect, useMemo, useRef, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  listWorkspaceSessionMessages,
  updateWorkspaceSession,
  type WorkspaceChatMessage,
} from "../../lib/workspace/client";
import type { WorkspaceSession } from "../../lib/workspace/model";
import { sortWorkspaceSessions } from "../../lib/workspace/model";
import {
  buildSearchSnippet,
  extractMessageSearchText,
  extractSessionTitleFromMessages,
  formatSessionUpdatedAt,
  normalizeSearchText,
  type SessionSearchDocument,
  type SessionSearchResult,
} from "../../lib/workspace/session-title-text";
import { WorkspaceQueryLibraryPanel } from "./workspace-query-library-panel";
import styles from "./workspace-shell.module.css";

type WorkspaceHistoryPaneProps = {
  sessions: WorkspaceSession[];
  activeSessionId: string | null;
  workspaceId: string;
  onInsert: (text: string) => boolean;
  onNewThread: () => void;
  onSelectSession: (sessionId: string) => void;
  onTogglePinSession: (session: WorkspaceSession) => void;
  onRenameSession: (session: WorkspaceSession) => void;
  onDeleteSession: (session: WorkspaceSession) => void;
  onRequestClose?: () => void;
};

export function WorkspaceHistoryPane({
  sessions,
  activeSessionId,
  workspaceId,
  onInsert,
  onNewThread,
  onSelectSession,
  onTogglePinSession,
  onRenameSession,
  onDeleteSession,
  onRequestClose,
}: WorkspaceHistoryPaneProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [openMenuSessionId, setOpenMenuSessionId] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchDocuments, setSearchDocuments] = useState<Record<string, SessionSearchDocument>>({});
  const [derivedSessionTitles, setDerivedSessionTitles] = useState<Record<string, string>>({});
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState("");
  const openMenuRef = useRef<HTMLDivElement | null>(null);
  const searchLoadingSessionIdsRef = useRef<Set<string>>(new Set());
  const titleLoadingSessionIdsRef = useRef<Set<string>>(new Set());
  const titleSyncAttemptedSessionIdsRef = useRef<Set<string>>(new Set());
  const sortedSessions = useMemo(() => sortWorkspaceSessions(sessions), [sessions]);
  const visibleSessions = useMemo(
    () =>
      sortedSessions.filter((session) => {
        const title = session.title?.trim() || derivedSessionTitles[session.id] || "";
        return Boolean(title);
      }),
    [derivedSessionTitles, sortedSessions],
  );
  const searchResults = useMemo<SessionSearchResult[]>(() => {
    const normalizedQuery = normalizeSearchText(searchQuery);

    if (!normalizedQuery) {
      return [];
    }

    return visibleSessions
      .map((session) => {
        const title = session.title?.trim() || derivedSessionTitles[session.id] || "";
        const documentText = searchDocuments[session.id]?.text ?? "";
        const searchableText = normalizeSearchText([title, documentText].join(" "));

        if (!searchableText.includes(normalizedQuery)) {
          return null;
        }

        return {
          id: session.id,
          title,
          description: buildSearchSnippet(session, searchQuery, documentText),
          updatedAtLabel: formatSessionUpdatedAt(locale, session.updated_at),
        };
      })
      .filter((result): result is SessionSearchResult => result !== null);
  }, [derivedSessionTitles, locale, searchDocuments, searchQuery, visibleSessions]);

  useEffect(() => {
    if (!openMenuSessionId) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;

      if (openMenuRef.current?.contains(target)) {
        return;
      }

      setOpenMenuSessionId(null);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpenMenuSessionId(null);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [openMenuSessionId]);

  useEffect(() => {
    setDerivedSessionTitles((previous) => {
      const activeSessionIds = new Set(sessions.map((session) => session.id));
      let changed = false;
      const nextTitles: Record<string, string> = {};

      for (const [sessionId, title] of Object.entries(previous)) {
        if (!activeSessionIds.has(sessionId)) {
          changed = true;
          continue;
        }

        nextTitles[sessionId] = title;
      }

      return changed ? nextTitles : previous;
    });
  }, [sessions]);

  useEffect(() => {
    if (!auth.token) {
      return;
    }

    const sessionsToLoad = sortedSessions.filter((session) => {
      return (
        !session.title?.trim() &&
        derivedSessionTitles[session.id] === undefined &&
        !titleLoadingSessionIdsRef.current.has(session.id)
      );
    });

    if (sessionsToLoad.length === 0) {
      return;
    }

    let cancelled = false;
    sessionsToLoad.forEach((session) => titleLoadingSessionIdsRef.current.add(session.id));

    Promise.allSettled(
      sessionsToLoad.map(async (session) => {
        const response = await listWorkspaceSessionMessages(auth.token!, session.id);

        return {
          sessionId: session.id,
          title: extractSessionTitleFromMessages(response.messages),
        };
      }),
    )
      .then((results) => {
        if (cancelled) {
          return;
        }

        setDerivedSessionTitles((previous) => {
          const nextTitles = { ...previous };

          results.forEach((result, index) => {
            const session = sessionsToLoad[index];
            nextTitles[session.id] =
              result.status === "fulfilled" ? result.value.title : previous[session.id] ?? "";
          });

          return nextTitles;
        });
      })
      .finally(() => {
        sessionsToLoad.forEach((session) => titleLoadingSessionIdsRef.current.delete(session.id));
      });

    return () => {
      cancelled = true;
    };
  }, [auth.token, derivedSessionTitles, sortedSessions]);

  useEffect(() => {
    if (!auth.token) {
      return;
    }

    const sessionsToSync = sortedSessions.filter((session) => {
      const resolvedTitle = derivedSessionTitles[session.id]?.trim();

      return (
        !session.title?.trim() &&
        Boolean(resolvedTitle) &&
        !titleSyncAttemptedSessionIdsRef.current.has(session.id)
      );
    });

    if (sessionsToSync.length === 0) {
      return;
    }

    let cancelled = false;
    sessionsToSync.forEach((session) => titleSyncAttemptedSessionIdsRef.current.add(session.id));

    Promise.allSettled(
      sessionsToSync.map(async (session) => {
        const title = derivedSessionTitles[session.id]!.trim();
        const updated = await updateWorkspaceSession(auth.token!, session.id, { title });

        return {
          sessionId: updated.id,
          title: updated.title?.trim() || title,
        };
      }),
    ).then((results) => {
      if (cancelled) {
        return;
      }

      setDerivedSessionTitles((previous) => {
        const nextTitles = { ...previous };

        results.forEach((result, index) => {
          if (result.status !== "fulfilled") {
            return;
          }

          const session = sessionsToSync[index];
          nextTitles[session.id] = result.value.title;
        });

        return nextTitles;
      });
    });

    return () => {
      cancelled = true;
    };
  }, [auth.token, derivedSessionTitles, sortedSessions]);

  useEffect(() => {
    setSearchDocuments((previous) => {
      const activeSessionIds = new Set(sessions.map((session) => session.id));
      let changed = false;
      const nextDocuments: Record<string, SessionSearchDocument> = {};

      for (const [sessionId, document] of Object.entries(previous)) {
        if (!activeSessionIds.has(sessionId)) {
          changed = true;
          continue;
        }

        nextDocuments[sessionId] = document;
      }

      return changed ? nextDocuments : previous;
    });
  }, [sessions]);

  useEffect(() => {
    if (!searchOpen || !auth.token) {
      return;
    }

    const sessionsToLoad = sortedSessions.filter((session) => {
      const cachedDocument = searchDocuments[session.id];
      return (
        cachedDocument?.updatedAt !== session.updated_at &&
        !searchLoadingSessionIdsRef.current.has(session.id)
      );
    });

    if (sessionsToLoad.length === 0) {
      setSearchLoading(searchLoadingSessionIdsRef.current.size > 0);
      return;
    }

    let cancelled = false;
    sessionsToLoad.forEach((session) => searchLoadingSessionIdsRef.current.add(session.id));
    setSearchLoading(true);
    setSearchError("");

    Promise.allSettled(
      sessionsToLoad.map(async (session) => {
        const response = await listWorkspaceSessionMessages(auth.token!, session.id);

        return {
          sessionId: session.id,
          updatedAt: session.updated_at,
          text: response.messages.map(extractMessageSearchText).filter(Boolean).join("\n"),
        };
      }),
    )
      .then((results) => {
        if (cancelled) {
          return;
        }

        let hadFailure = false;

        setSearchDocuments((previous) => {
          const nextDocuments = { ...previous };

          results.forEach((result, index) => {
            const session = sessionsToLoad[index];

            if (result.status === "fulfilled") {
              nextDocuments[result.value.sessionId] = {
                text: result.value.text,
                updatedAt: result.value.updatedAt,
              };
              return;
            }

            hadFailure = true;
            nextDocuments[session.id] = {
              text: previous[session.id]?.text ?? "",
              updatedAt: session.updated_at,
            };
          });

          return nextDocuments;
        });

        if (hadFailure) {
          setSearchError(formatUiMessage(locale, "workspaceSearchLoadError"));
        }
      })
      .finally(() => {
        sessionsToLoad.forEach((session) => searchLoadingSessionIdsRef.current.delete(session.id));

        if (!cancelled) {
          setSearchLoading(searchLoadingSessionIdsRef.current.size > 0);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [auth.token, locale, searchDocuments, searchOpen, sortedSessions]);

  function closeMenu() {
    setOpenMenuSessionId(null);
  }

  function closeSearch() {
    setSearchOpen(false);
    setSearchQuery("");
  }

  function handleNewThread() {
    closeMenu();
    onNewThread();
    onRequestClose?.();
  }

  function handleOpenSearch() {
    closeMenu();
    setSearchError("");
    setSearchQuery("");
    setSearchOpen(true);
  }

  function handleSearchSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const firstResult = searchResults[0];

    if (!firstResult) {
      return;
    }

    closeSearch();
    onSelectSession(firstResult.id);
    onRequestClose?.();
  }

  function handleSearchSelectSession(sessionId: string) {
    closeSearch();
    onSelectSession(sessionId);
    onRequestClose?.();
  }

  function handleSelectSession(sessionId: string) {
    closeMenu();
    onSelectSession(sessionId);
    onRequestClose?.();
  }

  return (
    <section className={styles.railPanel} aria-label={formatUiMessage(locale, "workspaceHistoryLabel")}>
      <div className={styles.railHeader}>
        <button className={styles.railPrimaryButton} type="button" onClick={handleNewThread}>
          <svg aria-hidden="true" className={styles.railActionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeWidth="2.2" />
          </svg>
          {formatUiMessage(locale, "workspaceNewThread")}
        </button>
        <button
          aria-haspopup="dialog"
          className={styles.railSecondaryButton}
          type="button"
          onClick={handleOpenSearch}
        >
          <svg aria-hidden="true" className={styles.railActionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="m21 21-4.35-4.35" strokeLinecap="round" strokeWidth="2" />
            <circle cx="11" cy="11" r="6" strokeWidth="2" />
          </svg>
          {formatUiMessage(locale, "workspaceHistorySearch")}
        </button>
      </div>

      <div className={styles.historyList}>
        {visibleSessions.length > 0 ? (
          visibleSessions.map((session) => {
            const title = session.title?.trim() || derivedSessionTitles[session.id] || "";
            const menuOpen = session.id === openMenuSessionId;
            const itemClassName = [
              session.id === activeSessionId ? styles.historyItemActive : styles.historyItem,
              menuOpen ? styles.historyItemMenuOpen : "",
            ]
              .filter(Boolean)
              .join(" ");

            return (
              <Fragment key={session.id}>
                <article className={itemClassName} data-testid="history-item" data-session-id={session.id}>
                  <button className={styles.historySelectButton} type="button" onClick={() => handleSelectSession(session.id)}>
                    <div className={styles.historyItemHeader}>
                      <div className={styles.historyItemTitle}>{title}</div>
                    </div>
                    {session.pinned ? (
                      <div className={styles.historyItemMeta}>{formatUiMessage(locale, "workspacePinnedSession")}</div>
                    ) : null}
                  </button>

                  <button
                    aria-expanded={menuOpen}
                    aria-haspopup="menu"
                    aria-label={formatUiMessage(locale, "workspaceRightRail.sessionActionsLabel", { title })}
                    className={styles.historyMenuTrigger}
                    type="button"
                    onClick={() => setOpenMenuSessionId((current) => (current === session.id ? null : session.id))}
                  >
                    <svg aria-hidden="true" className={styles.historyMenuTriggerIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path d="M12 6.75a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5ZM12 13.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5ZM12 19.75a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5Z" fill="currentColor" stroke="none" />
                    </svg>
                  </button>
                </article>

                {menuOpen ? (
                  <div className={styles.historyExpandedMenuShell} ref={openMenuRef}>
                    <div
                      className={styles.historyMenu}
                      role="menu"
                      aria-label={formatUiMessage(locale, "workspaceRightRail.sessionActionsLabel", { title })}
                    >
                      <button
                        className={styles.historyMenuButton}
                        role="menuitem"
                        type="button"
                        onClick={() => {
                          closeMenu();
                          onTogglePinSession(session);
                        }}
                      >
                        {formatUiMessage(
                          locale,
                          session.pinned ? "workspaceUnpinSessionAction" : "workspacePinSessionAction",
                          { title },
                        )}
                      </button>
                      <button
                        className={styles.historyMenuButton}
                        role="menuitem"
                        type="button"
                        onClick={() => {
                          closeMenu();
                          onRenameSession(session);
                        }}
                      >
                        {formatUiMessage(locale, "workspaceRenameSessionAction", { title })}
                      </button>
                      <button
                        className={styles.historyMenuButton}
                        role="menuitem"
                        type="button"
                        onClick={() => {
                          closeMenu();
                          onDeleteSession(session);
                        }}
                      >
                        {formatUiMessage(locale, "workspaceDeleteSessionAction", { title })}
                      </button>
                    </div>
                  </div>
                ) : null}
              </Fragment>
            );
          })
        ) : (
          <p className={styles.emptyState}>{formatUiMessage(locale, "workspaceNoSessionsMatch")}</p>
        )}
      </div>

      <WorkspaceQueryLibraryPanel workspaceId={workspaceId} onInsert={onInsert} />

      {searchOpen ? (
        <div className="dashboard-modal-backdrop" onClick={closeSearch} role="presentation">
          <section
            aria-label={formatUiMessage(locale, "workspaceSearchDialogLabel")}
            aria-modal="true"
            className="dashboard-search-modal"
            role="dialog"
            onClick={(event) => event.stopPropagation()}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                closeSearch();
              }
            }}
          >
            <div className="dashboard-search-header">
              <div>
                <h2 className="dashboard-modal-title">{formatUiMessage(locale, "workspaceSearchTitle")}</h2>
                <p className="dashboard-search-subtitle">{formatUiMessage(locale, "workspaceSearchSubtitle")}</p>
              </div>
              <button className="dashboard-action-button" type="button" onClick={closeSearch}>
                {formatUiMessage(locale, "dashboardCloseSearch")}
              </button>
            </div>
            <form className="dashboard-search-form" onSubmit={handleSearchSubmit}>
              <input
                aria-label={formatUiMessage(locale, "workspaceHistorySearch")}
                autoComplete="off"
                autoFocus
                className="app-input dashboard-search-input"
                id="workspace-search-query"
                name="query"
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder={formatUiMessage(locale, "workspaceSearchPlaceholder")}
                value={searchQuery}
              />
            </form>
            <div className="dashboard-search-results">
              {searchError ? <p className="dashboard-search-empty">{searchError}</p> : null}
              {searchQuery.trim().length === 0 ? (
                <p className="dashboard-search-empty">
                  {searchLoading
                    ? formatUiMessage(locale, "workspaceSearchLoading")
                    : formatUiMessage(locale, "workspaceSearchEmptyIdle")}
                </p>
              ) : searchResults.length === 0 ? (
                <p className="dashboard-search-empty">
                  {searchLoading
                    ? formatUiMessage(locale, "workspaceSearchLoading")
                    : formatUiMessage(locale, "workspaceSearchEmptyNoMatch")}
                </p>
              ) : (
                <>
                  {searchLoading ? (
                    <p className="dashboard-search-empty">{formatUiMessage(locale, "workspaceSearchLoading")}</p>
                  ) : null}
                  <ul
                    className="dashboard-search-list"
                    aria-label={formatUiMessage(locale, "workspaceSearchResultsLabel")}
                  >
                    {searchResults.map((result) => (
                      <li key={result.id} className="dashboard-search-item">
                        <button
                          aria-label={result.title}
                          className={`dashboard-search-link ${styles.searchResultButton}`}
                          type="button"
                          onClick={() => handleSearchSelectSession(result.id)}
                        >
                          <span className="dashboard-search-link-title">{result.title}</span>
                          {result.description ? (
                            <span className="dashboard-search-link-description">{result.description}</span>
                          ) : null}
                          <span className="dashboard-search-link-meta">{result.updatedAtLabel}</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}
