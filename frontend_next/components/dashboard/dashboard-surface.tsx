"use client";

import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

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
  type DashboardSortMode,
  type DashboardTab,
} from "../../lib/dashboard/model";
import {
  getFavoriteWorkspaceIds,
  updateFavoriteWorkspaceIds,
} from "../../lib/dashboard/preferences";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { DashboardHeader } from "./parts/dashboard-header";
import { DashboardSearchDialog } from "./parts/dashboard-search-dialog";
import { DashboardToolbar } from "./parts/dashboard-toolbar";
import {
  type DashboardViewMode,
  type DashboardWorkspaceView,
  formatWorkspaceTitle,
  mapWorkspace,
} from "./parts/dashboard-utils";
import { DashboardCreateTile, WorkspaceListItem } from "./parts/dashboard-workspace-items";

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