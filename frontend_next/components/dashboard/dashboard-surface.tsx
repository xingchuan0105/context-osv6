"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type FormEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";

import { ContextOsMark } from "../context-os-mark";
import { useAuth } from "../../lib/auth/context";
import {
  createWorkspace,
  deleteWorkspace,
  listWorkspaces,
  type DashboardWorkspace,
  updateWorkspace,
} from "../../lib/dashboard/client";
import { getDefaultWorkspaceTitle, markDefaultWorkspaceTitleUsed } from "../../lib/dashboard/default-title";
import {
  buildDashboardWorkspaceListState,
  type DashboardLocale,
  type DashboardSortMode,
  type DashboardTab,
  type DashboardWorkspaceInput,
} from "../../lib/dashboard/model";
import {
  getFavoriteWorkspaceIds,
  updateFavoriteWorkspaceIds,
} from "../../lib/dashboard/preferences";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";

type DashboardViewMode = "list" | "card";
type DashboardWorkspaceView = ReturnType<typeof buildDashboardWorkspaceListState>[number];
type DashboardWorkspaceItem = DashboardWorkspaceView & { documentCount: number };

function mapWorkspace(workspace: DashboardWorkspace): DashboardWorkspaceInput {
  return {
    id: workspace.workspace_id,
    title: workspace.title,
    name: workspace.name,
    description: workspace.description,
    createdAt: workspace.created_at,
    updatedAt: workspace.updated_at,
    ownerId: workspace.owner_id,
    statusSummary: workspace.status_summary,
  };
}

function formatWorkspaceTitle(locale: DashboardLocale, workspace: { title: string }) {
  const trimmedTitle = workspace.title.trim();
  return trimmedTitle.length > 0 ? trimmedTitle : formatUiMessage(locale, "dashboardUntitledWorkspace");
}

function getWorkspaceGlyph(title: string) {
  const normalized = title.toLowerCase();

  if (normalized.includes("interview")) {
    return "\u{1F3A4}";
  }

  if (normalized.includes("power") || normalized.includes("market")) {
    return "\u26A1";
  }

  if (normalized.includes("prospectus") || normalized.includes("analysis") || normalized.includes("research")) {
    return "\u{1F50E}";
  }

  if (normalized.includes("logic") || normalized.includes("network") || normalized.includes("rag")) {
    return "\u{1F916}";
  }

  if (normalized.includes("framework") || normalized.includes("flow")) {
    return "\u{1F504}";
  }

  return "\u{1F4C4}";
}

function getWorkspaceTone(title: string) {
  const tones = ["rose", "sage", "lavender", "amber"] as const;
  const normalized = title.trim().toLowerCase();
  let hash = 0;

  for (const char of normalized) {
    hash = (hash * 31 + char.charCodeAt(0)) % 9973;
  }

  return tones[hash % tones.length];
}

function formatWorkspaceSourceCount(locale: DashboardLocale, documentCount: number) {
  if (locale === "zh-CN") {
    return `${documentCount} \u4e2a\u6765\u6e90`;
  }

  return `${documentCount} source${documentCount === 1 ? "" : "s"}`;
}

function DashboardHeader({
  avatarInitial,
  locale,
}: {
  avatarInitial: string;
  locale: DashboardLocale;
}) {
  return (
    <header className="dashboard-header">
      <div className="dashboard-brand">
        <ContextOsMark className="dashboard-brand-mark" />
        <div>
          <div className="dashboard-brand-title">Context OS</div>
          <div className="dashboard-brand-subtitle">{formatUiMessage(locale, "dashboardBrandSubtitle")}</div>
        </div>
      </div>
      <div className="dashboard-header-links">
        <Link
          aria-label={formatUiMessage(locale, "dashboardSettingsLink")}
          className="dashboard-header-settings"
          href="/settings?tab=appearance"
        >
          <svg aria-hidden="true" className="dashboard-header-icon" fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" viewBox="0 0 24 24">
            <path d="M4 21V14M4 10V3M12 21V8M12 4V3M20 21v-9M20 8V3M1 14h6M9 8h6M17 18h6" />
          </svg>
          <span>{formatUiMessage(locale, "dashboardSettingsLink")}</span>
        </Link>
        <Link
          aria-label={formatUiMessage(locale, "dashboardProfileLink")}
          className="dashboard-avatar-link"
          href="/settings?tab=profile"
        >
          {avatarInitial}
        </Link>
      </div>
    </header>
  );
}

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

function DashboardToolbar({
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
        <button className="app-button-primary dashboard-create-button" disabled={creating} type="button" onClick={onCreate}>
          <svg aria-hidden="true" className="dashboard-create-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.4" />
          </svg>
          {formatUiMessage(locale, "dashboardNewWorkspace")}
        </button>
      </div>
    </div>
  );
}

function DashboardWorkspaceGlyph({ title }: { title: string }) {
  const tone = getWorkspaceTone(title);

  return (
    <div className={`dashboard-workspace-icon dashboard-workspace-icon-${tone}`} aria-hidden="true">
      {getWorkspaceGlyph(title)}
    </div>
  );
}

function DashboardWorkspaceTitle({
  title,
}: {
  title: string;
}) {
  return <div className="dashboard-workspace-title">{title}</div>;
}

function DashboardWorkspaceCardLink({
  children,
  className,
  title,
  workspaceId,
}: {
  children: ReactNode;
  className: string;
  title: string;
  workspaceId: string;
}) {
  return (
    <Link aria-label={title} className={className} href={`/dashboard/${workspaceId}`}>
      {children}
    </Link>
  );
}

function DashboardWorkspaceMeta({
  items,
}: {
  items: readonly string[];
}) {
  return (
    <div className="dashboard-workspace-meta">
      {items.map((item, index) => (
        <span key={`${item}-${index}`}>{item}</span>
      ))}
    </div>
  );
}

function DashboardCreateTile({
  creating,
  delay,
  onCreate,
}: {
  creating: boolean;
  delay: number;
  onCreate: () => void;
}) {
  const { locale } = useUiPreferences();

  return (
    <button className="dashboard-create-tile animate-card-enter" disabled={creating} style={{ animationDelay: `${delay}ms` }} type="button" onClick={onCreate}>
      <span className="dashboard-create-tile-icon" aria-hidden="true">
        +
      </span>
      <span className="dashboard-create-tile-title">{formatUiMessage(locale, "dashboardNewWorkspace")}</span>
    </button>
  );
}

function WorkspaceActions({
  layout,
  title,
  isFavorite,
  onFavoriteToggle,
  onRename,
  onDelete,
}: {
  layout: DashboardViewMode;
  title: string;
  isFavorite: boolean;
  onFavoriteToggle: () => void;
  onRename: () => void;
  onDelete: () => void;
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
    <div
      className={`dashboard-workspace-actions ${
        layout === "card" ? "dashboard-workspace-actions-card" : "dashboard-workspace-actions-list"
      }`}
      ref={menuRef}
    >
      <button
        aria-expanded={menuOpen}
        aria-haspopup="menu"
        aria-label={formatUiMessage(locale, "workspaceRightRail.sessionActionsLabel", { title })}
        className="dashboard-menu-trigger"
        type="button"
        onClick={() => setMenuOpen((current) => !current)}
      >
        <svg aria-hidden="true" className="dashboard-menu-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path d="M12 6h.01M12 12h.01M12 18h.01" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2.4" />
        </svg>
      </button>
      {menuOpen ? (
        <div
          aria-label={formatUiMessage(locale, "workspaceRightRail.sessionActionsLabel", { title })}
          className="dashboard-action-menu"
          role="menu"
        >
          <button
            className="dashboard-action-menu-item"
            role="menuitem"
            type="button"
            onClick={() => {
              setMenuOpen(false);
              onFavoriteToggle();
            }}
          >
            {formatUiMessage(locale, isFavorite ? "dashboardActionUnfavorite" : "dashboardActionFavorite")}
          </button>
          <button
            className="dashboard-action-menu-item"
            role="menuitem"
            type="button"
            onClick={() => {
              setMenuOpen(false);
              onRename();
            }}
          >
            {formatUiMessage(locale, "dashboardActionRename")}
          </button>
          <button
            className="dashboard-action-menu-item dashboard-action-menu-item-danger"
            role="menuitem"
            type="button"
            onClick={() => {
              setMenuOpen(false);
              onDelete();
            }}
          >
            {formatUiMessage(locale, "dashboardActionDelete")}
          </button>
        </div>
      ) : null}
    </div>
  );
}

function WorkspaceListItem({
  mode,
  workspace,
  index,
  onFavoriteToggle,
  onRename,
  onDelete,
}: {
  mode: DashboardViewMode;
  workspace: DashboardWorkspaceItem;
  index: number;
  onFavoriteToggle: () => void;
  onRename: () => void;
  onDelete: () => void;
}) {
  const { locale } = useUiPreferences();
  const title = workspace.title;
  const sourcesLabel = formatWorkspaceSourceCount(locale, workspace.documentCount);

  if (mode === "list") {
    return (
      <li className="dashboard-list-item animate-card-enter" style={{ animationDelay: `${index * 50}ms` }}>
        <article className="dashboard-workspace-card">
          <DashboardWorkspaceCardLink
            className="dashboard-workspace-card-link dashboard-workspace-card-link-list"
            title={title}
            workspaceId={workspace.id}
          >
            <div className="dashboard-list-row">
              <div className="dashboard-list-title-cell">
                <DashboardWorkspaceGlyph title={title} />
                <div className="dashboard-workspace-copy">
                  <DashboardWorkspaceTitle title={title} />
                </div>
              </div>

              <div className="dashboard-list-meta-cell">{sourcesLabel}</div>
              <div className="dashboard-list-meta-cell">{workspace.dateLabel}</div>

              <div className="dashboard-list-role-cell">
                <span className="dashboard-list-role-copy">{workspace.roleLabel}</span>
                <span aria-hidden="true" className="dashboard-list-action-slot" />
              </div>
            </div>
          </DashboardWorkspaceCardLink>
          <WorkspaceActions
            isFavorite={workspace.isFavorite}
            layout="list"
            onDelete={onDelete}
            onFavoriteToggle={onFavoriteToggle}
            onRename={onRename}
            title={title}
          />
        </article>
      </li>
    );
  }

  return (
    <li className={`${mode === "card" ? "dashboard-card-item" : "dashboard-list-item"} animate-card-enter`} role={mode === "card" ? "gridcell" : undefined} style={{ animationDelay: `${index * 50}ms` }}>
      <article className="dashboard-workspace-card">
        <DashboardWorkspaceCardLink
          className="dashboard-workspace-card-link dashboard-workspace-card-link-card"
          title={title}
          workspaceId={workspace.id}
        >
          <div className="dashboard-workspace-head">
            <div className="dashboard-workspace-head-top">
              <DashboardWorkspaceGlyph title={title} />
              <span aria-hidden="true" className="dashboard-workspace-action-slot" />
            </div>
            <div className="dashboard-workspace-copy">
              <DashboardWorkspaceTitle title={title} />
              <DashboardWorkspaceMeta items={[sourcesLabel]} />
            </div>
          </div>
          <div className="dashboard-workspace-footer">
            <span className="dashboard-workspace-footer-meta">{workspace.dateLabel}</span>
            <span className="dashboard-workspace-footer-meta">{workspace.roleLabel}</span>
          </div>
        </DashboardWorkspaceCardLink>
        <WorkspaceActions
          isFavorite={workspace.isFavorite}
          layout="card"
          onDelete={onDelete}
          onFavoriteToggle={onFavoriteToggle}
          onRename={onRename}
          title={title}
        />
      </article>
    </li>
  );
}

function DashboardSearchDialog({
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

export function DashboardSurface() {
  const router = useRouter();
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [workspaces, setWorkspaces] = useState<DashboardWorkspace[]>([]);
  const [favoriteIds, setFavoriteIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [activeTab, setActiveTab] = useState<DashboardTab>("all");
  const [sortMode, setSortMode] = useState<DashboardSortMode>("recent");
  const [viewMode, setViewMode] = useState<DashboardViewMode>("card");
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [creatingWorkspace, setCreatingWorkspace] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    let cancelled = false;

    async function loadDashboard() {
      if (!auth.token) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");

      const [workspaceResult, favoriteResult] = await Promise.allSettled([
        listWorkspaces(auth.token),
        getFavoriteWorkspaceIds(auth.token),
      ]);

      if (cancelled) {
        return;
      }

      if (workspaceResult.status === "fulfilled") {
        setWorkspaces(workspaceResult.value.workspaces);
      } else {
        setError(formatUiMessage(locale, "dashboardLoadError"));
      }

      if (favoriteResult.status === "fulfilled") {
        setFavoriteIds(favoriteResult.value);
      }

      setLoading(false);
    }

    loadDashboard();

    return () => {
      cancelled = true;
    };
  }, [auth.token, locale, refreshKey]);

  useEffect(() => {
    function handleStorage(event: StorageEvent) {
      if (event.key === "avrag.workspace-renamed.v1") {
        setRefreshKey((k) => k + 1);
      }
    }

    window.addEventListener("storage", handleStorage);
    return () => window.removeEventListener("storage", handleStorage);
  }, []);

  const currentUserId = auth.user?.id ?? "";
  const workspaceInputs = useMemo(() => workspaces.map(mapWorkspace), [workspaces]);
  const workspaceDocumentCountById = useMemo(
    () => new Map(workspaces.map((workspace) => [workspace.workspace_id, workspace.document_count])),
    [workspaces],
  );

  const visibleWorkspaces = useMemo(
    () =>
      buildDashboardWorkspaceListState(workspaceInputs, {
        locale,
        currentUserId,
        favoriteIds,
        tab: activeTab,
        sort: sortMode,
        query: "",
      }).map((workspace) => ({
        ...workspace,
        documentCount: workspaceDocumentCountById.get(workspace.id) ?? 0,
      })),
    [activeTab, currentUserId, favoriteIds, locale, sortMode, workspaceDocumentCountById, workspaceInputs],
  );

  const avatarInitial = (auth.user?.full_name?.trim() || auth.user?.email?.trim() || "U").slice(0, 1).toUpperCase();
  const sourcesColumnLabel = locale === "zh-CN" ? "\u6765\u6e90" : "Sources";

  async function handleCreateWorkspace() {
    if (!auth.token) {
      setError(formatUiMessage(locale, "dashboardLoginRequired"));
      return;
    }

    if (creatingWorkspace) {
      return;
    }

    const nextName = getDefaultWorkspaceTitle(locale, "");
    setCreatingWorkspace(true);
    setSearchOpen(false);

    try {
      const response = await createWorkspace(auth.token, {
        name: nextName,
        description: "",
      });

      setWorkspaces((current) => [response.workspace, ...current]);
      markDefaultWorkspaceTitleUsed(locale, "");
      router.push(`/dashboard/${response.workspace.workspace_id}`);
    } catch (submitError) {
      setError(String(submitError));
    } finally {
      setCreatingWorkspace(false);
    }
  }

  async function toggleFavorite(workspaceId: string) {
    if (!auth.token) {
      setError(formatUiMessage(locale, "dashboardLoginRequired"));
      return;
    }

    const previous = favoriteIds;
    const next = previous.includes(workspaceId)
      ? previous.filter((item) => item !== workspaceId)
      : [...previous, workspaceId];

    setFavoriteIds(next);

    try {
      const updated = await updateFavoriteWorkspaceIds(auth.token, next);
      setFavoriteIds(updated);
    } catch (toggleError) {
      setFavoriteIds(previous);
      setError(String(toggleError));
    }
  }

  async function renameWorkspace(workspace: DashboardWorkspaceView) {
    const currentTitle = formatWorkspaceTitle(locale, workspace);
    const nextTitle = window.prompt(formatUiMessage(locale, "dashboardPromptRename", { title: currentTitle }), currentTitle);

    if (nextTitle === null) {
      return;
    }

    const trimmedTitle = nextTitle.trim();
    if (!trimmedTitle || trimmedTitle === currentTitle || !auth.token) {
      return;
    }

    const sourceWorkspace = workspaces.find((item) => item.workspace_id === workspace.id);

    try {
      const response = await updateWorkspace(auth.token, workspace.id, {
        name: trimmedTitle,
        description: sourceWorkspace?.description ?? "",
      });

      setWorkspaces((current) =>
        current.map((item) => (item.workspace_id === workspace.id ? response.workspace : item)),
      );
    } catch (renameError) {
      setError(String(renameError));
    }
  }

  async function deleteWorkspaceById(workspace: DashboardWorkspaceView) {
    const currentTitle = formatWorkspaceTitle(locale, workspace);
    if (!window.confirm(formatUiMessage(locale, "dashboardConfirmDelete", { title: currentTitle }))) {
      return;
    }

    if (!auth.token) {
      setError(formatUiMessage(locale, "dashboardLoginRequired"));
      return;
    }

    try {
      await deleteWorkspace(auth.token, workspace.id);
      const nextFavoriteIds = favoriteIds.filter((item) => item !== workspace.id);
      setWorkspaces((current) => current.filter((item) => item.workspace_id !== workspace.id));
      setFavoriteIds(nextFavoriteIds);
      if (favoriteIds.includes(workspace.id)) {
        const updatedFavorites = await updateFavoriteWorkspaceIds(auth.token, nextFavoriteIds);
        setFavoriteIds(updatedFavorites);
      }
    } catch (deleteError) {
      setError(String(deleteError));
    }
  }


  return (
    <main className="dashboard-shell">
      <DashboardHeader avatarInitial={avatarInitial} locale={locale} />

      <section className="dashboard-main">
        <DashboardToolbar
          activeTab={activeTab}
          creating={creatingWorkspace}
          onCreate={() => void handleCreateWorkspace()}
          onTabChange={setActiveTab}
          onSearch={() => setSearchOpen(true)}
          onSortChange={setSortMode}
          onViewChange={setViewMode}
          sortMode={sortMode}
          viewMode={viewMode}
        />

        <div className="dashboard-heading-row">
          <h1 className="dashboard-heading">
            {activeTab === "mine"
              ? formatUiMessage(locale, "dashboardHeadingMine")
              : activeTab === "favorites"
                ? formatUiMessage(locale, "dashboardHeadingFavorites")
                : formatUiMessage(locale, "dashboardHeadingAll")}
          </h1>
          <p className="dashboard-heading-meta">{formatUiMessage(locale, "dashboardHeadingCount", { count: visibleWorkspaces.length })}</p>
        </div>

        {error ? <p className="app-notice-banner dashboard-error">{error}</p> : null}

        {loading ? (
          <section className="dashboard-empty-state">
            <p>{formatUiMessage(locale, "dashboardLoading")}</p>
          </section>
        ) : visibleWorkspaces.length === 0 ? (
          <section className="dashboard-empty-state">
            <h2>{activeTab === "favorites" ? formatUiMessage(locale, "dashboardEmptyFavoritesTitle") : formatUiMessage(locale, "dashboardEmptyAllTitle")}</h2>
            <p>{formatUiMessage(locale, "dashboardEmptyBody")}</p>
            <button className="app-button-primary" disabled={creatingWorkspace} type="button" onClick={() => void handleCreateWorkspace()}>
              {formatUiMessage(locale, "dashboardCreateFirst")}
            </button>
          </section>
        ) : viewMode === "card" ? (
          <section aria-label={formatUiMessage(locale, "dashboardViewGridLabel")} className="dashboard-grid" role="grid">
            <DashboardCreateTile creating={creatingWorkspace} delay={visibleWorkspaces.length * 50} onCreate={() => void handleCreateWorkspace()} />
            {visibleWorkspaces.map((workspace, index) => {
              return (
                <WorkspaceListItem
                  key={workspace.id}
                  index={index}
                  mode="card"
                  onDelete={() => void deleteWorkspaceById(workspace)}
                  onFavoriteToggle={() => void toggleFavorite(workspace.id)}
                  onRename={() => void renameWorkspace(workspace)}
                  workspace={workspace}
                />
              );
            })}
          </section>
        ) : (
          <section className="dashboard-list-shell">
            <div className="dashboard-list-header" aria-hidden="true">
              <div>{formatUiMessage(locale, "dashboardWorkspaceNameField")}</div>
              <div>{sourcesColumnLabel}</div>
              <div>{formatUiMessage(locale, "dashboardCreatedAtColumn")}</div>
              <div>{formatUiMessage(locale, "dashboardRoleColumn")}</div>
            </div>
            <ul aria-label={formatUiMessage(locale, "dashboardListLabel")} className="dashboard-list">
              {visibleWorkspaces.map((workspace, index) => (
                <WorkspaceListItem
                  key={workspace.id}
                  index={index}
                  mode="list"
                  onDelete={() => void deleteWorkspaceById(workspace)}
                  onFavoriteToggle={() => void toggleFavorite(workspace.id)}
                  onRename={() => void renameWorkspace(workspace)}
                  workspace={workspace}
                />
              ))}
            </ul>
          </section>
        )}
      </section>

      {searchOpen ? (
        <DashboardSearchDialog
          currentUserId={currentUserId}
          favoriteIds={favoriteIds}
          onClose={() => setSearchOpen(false)}
          onNavigate={(workspaceId) => {
            setSearchOpen(false);
            router.push(`/dashboard/${workspaceId}`);
          }}
          query={searchQuery}
          setQuery={setSearchQuery}
          workspaces={workspaces}
        />
      ) : null}
    </main>
  );
}
