"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, useEffect, useState } from "react";

import { useAuth } from "../../../lib/auth/context";
import { formatUiMessage, type UiMessageKey } from "../../../lib/i18n/messages";
import {
  searchProductIndex,
  type GlobalSearchResponse,
} from "../../../lib/search/client";
import { useUiPreferences } from "../../../lib/ui-preferences";
import { workspaceSourceHref } from "../../../lib/workspace/session-url";

const SEARCH_MIN_CHARS = 2;
const SEARCH_DEBOUNCE_MS = 220;

type SearchGroup = "sessions" | "workspaces" | "sources";

type SearchRow = {
  id: string;
  group: SearchGroup;
  title: string;
  /** Right-aligned meta (workspace name / date). */
  meta: string;
  /** Secondary line under the title. */
  description: string;
  href: string;
};

const GROUP_ORDER: SearchGroup[] = ["sessions", "workspaces", "sources"];

const GROUP_LABEL: Record<SearchGroup, UiMessageKey> = {
  sessions: "commandPalette.group.sessions",
  workspaces: "commandPalette.group.workspaces",
  sources: "commandPalette.group.sources",
};

const EMPTY_HITS: GlobalSearchResponse = { workspaces: [], sessions: [], sources: [] };

/**
 * Dashboard global search (Grok-style modal): one keyword hits the product
 * index — sessions (incl. chat body), workspaces, and source documents.
 */
export function DashboardSearchDialog({ onClose }: { onClose: () => void }) {
  const auth = useAuth();
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<GlobalSearchResponse | null>(null);
  const [loading, setLoading] = useState(false);

  const trimmed = query.trim();
  const searching = trimmed.length >= SEARCH_MIN_CHARS;

  useEffect(() => {
    if (!searching || !auth.token) {
      setHits(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    const timer = window.setTimeout(() => {
      void searchProductIndex(auth.token as string, trimmed)
        .then((response) => {
          if (!cancelled) {
            setHits(response);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setHits(EMPTY_HITS);
          }
        })
        .finally(() => {
          if (!cancelled) {
            setLoading(false);
          }
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [auth.token, searching, trimmed]);

  const rows: SearchRow[] = [];
  if (hits) {
    for (const session of hits.sessions.slice(0, 20)) {
      const title =
        session.title?.trim() || formatUiMessage(locale, "commandPalette.sessionUntitled");
      rows.push({
        id: `sess-${session.id}`,
        group: "sessions",
        title,
        meta: "",
        description: "",
        href: `/dashboard/${session.workspace_id}?session=${encodeURIComponent(session.id)}`,
      });
    }
    for (const ws of hits.workspaces.slice(0, 20)) {
      const title = (ws.title || ws.name || ws.id).trim();
      rows.push({
        id: `ws-${ws.id}`,
        group: "workspaces",
        title,
        meta: "",
        description: ws.description?.trim() ?? "",
        href: `/dashboard/${ws.id}`,
      });
    }
    for (const source of hits.sources.slice(0, 15)) {
      const name = (source.file_name || source.title || source.id).trim();
      rows.push({
        id: `src-${source.id}`,
        group: "sources",
        title: name,
        meta: source.workspace_name?.trim() ?? "",
        description: "",
        href: workspaceSourceHref(source.workspace_id, source.id),
      });
    }
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    // Enter jumps to the first hit.
    const first = rows[0];
    if (first) {
      onClose();
      router.push(first.href);
    }
  }

  return (
    <div className="dashboard-modal-backdrop" onClick={onClose} role="presentation">
      <section
        aria-label={formatUiMessage(locale, "dashboardSearchDialogLabel")}
        className="dashboard-search-modal"
        role="dialog"
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          }
        }}
      >
        {/* Grok-style: bare input on top, results scroll below; Esc/backdrop closes. */}
        <form className="dashboard-search-form" onSubmit={handleSubmit}>
          <input
            aria-label={formatUiMessage(locale, "dashboardSearchLabel")}
            autoComplete="off"
            autoFocus
            className="app-input dashboard-search-input"
            id="dashboard-search-query"
            name="query"
            onChange={(event) => setQuery(event.target.value)}
            placeholder={formatUiMessage(locale, "dashboardSearchPlaceholder")}
            value={query}
          />
        </form>
        <div className="dashboard-search-results">
          {!searching ? (
            <p className="dashboard-search-empty">
              {formatUiMessage(locale, "dashboardSearchEmptyIdle")}
            </p>
          ) : loading && !hits ? (
            <p className="dashboard-search-empty">
              {formatUiMessage(locale, "commandPalette.loadingSearch")}
            </p>
          ) : rows.length === 0 ? (
            <p className="dashboard-search-empty">
              {formatUiMessage(locale, "dashboardSearchEmptyNoMatch")}
            </p>
          ) : (
            GROUP_ORDER.map((group) => {
              const groupRows = rows.filter((row) => row.group === group);
              if (groupRows.length === 0) {
                return null;
              }
              return (
                <div key={group}>
                  <p className="dashboard-search-group-label">
                    {formatUiMessage(locale, GROUP_LABEL[group])}
                  </p>
                  <ul
                    className="dashboard-search-list"
                    aria-label={formatUiMessage(locale, GROUP_LABEL[group])}
                  >
                    {groupRows.map((row) => (
                      <li key={row.id} className="dashboard-search-item">
                        <Link
                          aria-label={row.title}
                          className="dashboard-search-link"
                          href={row.href}
                          onClick={onClose}
                        >
                          <span className="dashboard-search-link-title">{row.title}</span>
                          {row.description ? (
                            <span className="dashboard-search-link-description">
                              {row.description}
                            </span>
                          ) : null}
                          <span className="dashboard-search-link-meta">{row.meta}</span>
                        </Link>
                      </li>
                    ))}
                  </ul>
                </div>
              );
            })
          )}
        </div>
      </section>
    </div>
  );
}
