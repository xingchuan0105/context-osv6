"use client";

import Link from "next/link";
import { type FormEvent, useMemo } from "react";

import { type DashboardWorkspace } from "../../../lib/dashboard/client";
import {
  buildDashboardWorkspaceListState,
} from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";
import { mapWorkspace } from "./dashboard-utils";

export function DashboardSearchDialog({
  currentUserId,
  favoriteIds,
  onClose,
  onNavigate,
  query,
  setQuery,
  workspaces,
}: {
  currentUserId: string;
  favoriteIds: readonly string[];
  onClose: () => void;
  onNavigate: (workspaceId: string) => void;
  query: string;
  setQuery: (value: string) => void;
  workspaces: readonly DashboardWorkspace[];
}) {
  const { locale } = useUiPreferences();
  const searchResults = useMemo(
    () =>
      buildDashboardWorkspaceListState(
        workspaces.map(mapWorkspace),
        {
          locale,
          currentUserId,
          favoriteIds,
          tab: "all",
          sort: "recent",
          query,
        },
      ),
    [currentUserId, favoriteIds, locale, query, workspaces],
  );

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const firstResult = searchResults[0];
    if (firstResult) {
      onNavigate(firstResult.id);
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
        <div className="dashboard-search-header">
          <div>
            <h2 className="dashboard-modal-title">{formatUiMessage(locale, "dashboardSearchTitle")}</h2>
            <p className="dashboard-search-subtitle">{formatUiMessage(locale, "dashboardSearchSubtitle")}</p>
          </div>
          <button className="dashboard-action-button" type="button" onClick={onClose}>
            {formatUiMessage(locale, "dashboardCloseSearch")}
          </button>
        </div>
        <form className="dashboard-search-form" onSubmit={handleSubmit}>
          <input
            aria-label={formatUiMessage(locale, "dashboardSearchLabel")}
            autoComplete="off"
            className="app-input dashboard-search-input"
            id="dashboard-search-query"
            name="query"
            onChange={(event) => setQuery(event.target.value)}
            placeholder={formatUiMessage(locale, "dashboardSearchPlaceholder")}
            value={query}
          />
        </form>
        <div className="dashboard-search-results">
          {query.trim().length === 0 ? (
            <p className="dashboard-search-empty">{formatUiMessage(locale, "dashboardSearchEmptyIdle")}</p>
          ) : searchResults.length === 0 ? (
            <p className="dashboard-search-empty">{formatUiMessage(locale, "dashboardSearchEmptyNoMatch")}</p>
          ) : (
            <ul className="dashboard-search-list" aria-label={formatUiMessage(locale, "dashboardSearchResultsLabel")}>
              {searchResults.map((workspace) => (
                <li key={workspace.id} className="dashboard-search-item">
                  <Link
                    aria-label={workspace.title}
                    className="dashboard-search-link"
                    href={`/dashboard/${workspace.id}`}
                  >
                    <span className="dashboard-search-link-title">{workspace.title}</span>
                    <span className="dashboard-search-link-description">{workspace.description}</span>
                    <span className="dashboard-search-link-meta">{workspace.dateLabel}</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>
    </div>
  );
}