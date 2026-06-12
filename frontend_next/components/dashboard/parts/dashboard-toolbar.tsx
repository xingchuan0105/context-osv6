"use client";

import { useEffect, useRef, useState } from "react";

import {
  type DashboardLocale,
  type DashboardSortMode,
  type DashboardTab,
} from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";
import type { DashboardViewMode } from "./dashboard-utils";

function DashboardTabs({
  activeTab,
  onChange,
}: {
  activeTab: DashboardTab;
  onChange: (tab: DashboardTab) => void;
}) {
  const { locale } = useUiPreferences();

  return (
    <nav className="dashboard-tabs" aria-label={formatUiMessage(locale, "dashboardTabsLabel")}>
      <button
        aria-pressed={activeTab === "all"}
        className="dashboard-tab-button"
        type="button"
        onClick={() => onChange("all")}
      >
        {formatUiMessage(locale, "dashboardTabAll")}
      </button>
      <button
        aria-pressed={activeTab === "mine"}
        className="dashboard-tab-button"
        type="button"
        onClick={() => onChange("mine")}
      >
        {formatUiMessage(locale, "dashboardTabMine")}
      </button>
      <button
        aria-pressed={activeTab === "favorites"}
        className="dashboard-tab-button"
        type="button"
        onClick={() => onChange("favorites")}
      >
        {formatUiMessage(locale, "dashboardTabFavorites")}
      </button>
    </nav>
  );
}

function DashboardSortMenu({
  onChange,
  sortMode,
}: {
  onChange: (sortMode: DashboardSortMode) => void;
  sortMode: DashboardSortMode;
}) {
  const { locale } = useUiPreferences();
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!menuOpen) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;

      if (menuRef.current?.contains(target)) {
        return;
      }

      setMenuOpen(false);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setMenuOpen(false);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [menuOpen]);

  return (
    <div className="dashboard-sort-menu" ref={menuRef}>
      <button
        aria-expanded={menuOpen}
        aria-haspopup="menu"
        className="dashboard-sort-trigger"
        type="button"
        onClick={() => setMenuOpen((current) => !current)}
      >
        <span>
          {sortMode === "recent"
            ? formatUiMessage(locale, "dashboardSortRecent")
            : formatUiMessage(locale, "dashboardSortTitle")}
        </span>
        <svg aria-hidden="true" className="dashboard-sort-chevron" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="m7 10 5 5 5-5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.2" />
        </svg>
      </button>
      {menuOpen ? (
        <div className="dashboard-action-menu dashboard-sort-dropdown" role="menu">
          <button
            aria-pressed={sortMode === "recent"}
            className="dashboard-action-menu-item"
            role="menuitem"
            type="button"
            onClick={() => {
              setMenuOpen(false);
              onChange("recent");
            }}
          >
            {formatUiMessage(locale, "dashboardSortRecent")}
          </button>
          <button
            aria-pressed={sortMode === "title"}
            className="dashboard-action-menu-item"
            role="menuitem"
            type="button"
            onClick={() => {
              setMenuOpen(false);
              onChange("title");
            }}
          >
            {formatUiMessage(locale, "dashboardSortTitle")}
          </button>
        </div>
      ) : null}
    </div>
  );
}

export function DashboardToolbar({
  activeTab,
  creating,
  onCreate,
  onTabChange,
  onSearch,
  onSortChange,
  onViewChange,
  sortMode,
  viewMode,
}: {
  activeTab: DashboardTab;
  creating: boolean;
  onCreate: () => void;
  onTabChange: (tab: DashboardTab) => void;
  onSearch: () => void;
  onSortChange: (sortMode: DashboardSortMode) => void;
  onViewChange: (viewMode: DashboardViewMode) => void;
  sortMode: DashboardSortMode;
  viewMode: DashboardViewMode;
}) {
  const { locale } = useUiPreferences();

  return (
    <div className="dashboard-toolbar">
      <DashboardTabs activeTab={activeTab} onChange={onTabChange} />
      <div className="dashboard-toolbar-actions">
        <button
          aria-label={formatUiMessage(locale, "dashboardToolbarSearch")}
          className="dashboard-icon-button"
          type="button"
          onClick={onSearch}
        >
          <svg aria-hidden="true" className="dashboard-control-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="m21 21-4.35-4.35" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.2" />
            <circle cx="11" cy="11" r="6" strokeWidth="2.2" />
          </svg>
          <span className="dashboard-sr-only">{formatUiMessage(locale, "dashboardToolbarSearch")}</span>
        </button>
        <div className="dashboard-segmented-control" aria-label={formatUiMessage(locale, "dashboardViewModeLabel")}>
          <button
            aria-pressed={viewMode === "card"}
            className="dashboard-segmented-button"
            type="button"
            onClick={() => onViewChange("card")}
          >
            <svg aria-hidden="true" className="dashboard-control-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <rect x="4" y="4" width="6" height="6" rx="1.4" strokeWidth="2" />
              <rect x="14" y="4" width="6" height="6" rx="1.4" strokeWidth="2" />
              <rect x="4" y="14" width="6" height="6" rx="1.4" strokeWidth="2" />
              <rect x="14" y="14" width="6" height="6" rx="1.4" strokeWidth="2" />
            </svg>
            <span className="dashboard-sr-only">{formatUiMessage(locale, "dashboardViewCard")}</span>
          </button>
          <button
            aria-pressed={viewMode === "list"}
            className="dashboard-segmented-button"
            type="button"
            onClick={() => onViewChange("list")}
          >
            <svg aria-hidden="true" className="dashboard-control-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M5 7h14M5 12h14M5 17h14" strokeLinecap="round" strokeWidth="2.2" />
            </svg>
            <span className="dashboard-sr-only">{formatUiMessage(locale, "dashboardViewList")}</span>
          </button>
        </div>
        <DashboardSortMenu onChange={onSortChange} sortMode={sortMode} />
        <button className="app-button-primary dashboard-create-button" data-testid="dashboard-create-workspace" disabled={creating} type="button" onClick={onCreate}>
          <svg aria-hidden="true" className="dashboard-create-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.4" />
          </svg>
          {formatUiMessage(locale, "dashboardNewWorkspace")}
        </button>
      </div>
    </div>
  );
}