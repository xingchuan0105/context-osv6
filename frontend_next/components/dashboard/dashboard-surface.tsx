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
import { ProductChromeFooter } from "../product-chrome-footer";
import { DashboardHeader } from "./parts/dashboard-header";
import { DashboardSearchDialog } from "./parts/dashboard-search-dialog";
import { DashboardSkeleton } from "./dashboard-skeleton";
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
  const [error, setError] = useState<{ message: string; retry: (() => void) | null } | null>(null);
  const [renameTarget, setRenameTarget] = useState<DashboardWorkspaceView | null>(null);
  const [renameTitle, setRenameTitle] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<DashboardWorkspaceView | null>(null);
  const [activeTab, setActiveTab] = useState<DashboardTab>("all");
  const [sortMode, setSortMode] = useState<DashboardSortMode>("recent");
  const [viewMode, setViewMode] = useState<DashboardViewMode>("card");
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [creatingWorkspace, setCreatingWorkspace] = useState(false);
  const [renameSubmitting, setRenameSubmitting] = useState(false);
  const [deleteSubmitting, setDeleteSubmitting] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    let cancelled = false;

    async function loadDashboard() {
      if (!auth.token) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError(null);

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
        console.error(workspaceResult.reason);
        setError({
          message: formatUiMessage(locale, "dashboardLoadError"),
          retry: () => setRefreshKey((key) => key + 1),
        });
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

  function reportError(actionError: unknown, retry: (() => void) | null) {
    console.error(actionError);
    setError({ message: formatUiMessage(locale, "dashboardActionFailed"), retry });
  }

  async function handleCreateWorkspace() {
    if (!auth.token) {
      setError({ message: formatUiMessage(locale, "dashboardLoginRequired"), retry: null });
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
      reportError(submitError, () => void handleCreateWorkspace());
    } finally {
      setCreatingWorkspace(false);
    }
  }

  async function toggleFavorite(workspaceId: string) {
    if (!auth.token) {
      setError({ message: formatUiMessage(locale, "dashboardLoginRequired"), retry: null });
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
      reportError(toggleError, () => void toggleFavorite(workspaceId));
    }
  }

  function openRenameDialog(workspace: DashboardWorkspaceView) {
    setRenameTarget(workspace);
    setRenameTitle(formatWorkspaceTitle(locale, workspace));
  }

  function dismissRenameDialog() {
    setRenameTarget(null);
    setRenameTitle("");
  }

  async function submitRenameWorkspace() {
    if (!renameTarget) {
      return;
    }

    const currentTitle = formatWorkspaceTitle(locale, renameTarget);
    const trimmedTitle = renameTitle.trim();

    if (!trimmedTitle || trimmedTitle === currentTitle) {
      dismissRenameDialog();
      return;
    }

    if (!auth.token) {
      setError({ message: formatUiMessage(locale, "dashboardLoginRequired"), retry: null });
      return;
    }

    const sourceWorkspace = workspaces.find((item) => item.workspace_id === renameTarget.id);

    if (renameSubmitting) {
      return;
    }

    setRenameSubmitting(true);

    try {
      const response = await updateWorkspace(auth.token, renameTarget.id, {
        name: trimmedTitle,
        description: sourceWorkspace?.description ?? "",
      });

      setWorkspaces((current) =>
        current.map((item) => (item.workspace_id === renameTarget.id ? response.workspace : item)),
      );
      dismissRenameDialog();
    } catch (renameError) {
      // Retry re-opens the confirmation dialog with the attempted target/title
      // instead of re-running the mutation behind the user's back.
      reportError(renameError, () => {
        setRenameTarget(renameTarget);
        setRenameTitle(trimmedTitle);
      });
    } finally {
      setRenameSubmitting(false);
    }
  }

  async function confirmDeleteWorkspace() {
    if (!deleteTarget) {
      return;
    }

    if (!auth.token) {
      setError({ message: formatUiMessage(locale, "dashboardLoginRequired"), retry: null });
      return;
    }

    if (deleteSubmitting) {
      return;
    }

    setDeleteSubmitting(true);

    try {
      await deleteWorkspace(auth.token, deleteTarget.id);
      const nextFavoriteIds = favoriteIds.filter((item) => item !== deleteTarget.id);
      setWorkspaces((current) => current.filter((item) => item.workspace_id !== deleteTarget.id));
      setFavoriteIds(nextFavoriteIds);
      if (favoriteIds.includes(deleteTarget.id)) {
        const updatedFavorites = await updateFavoriteWorkspaceIds(auth.token, nextFavoriteIds);
        setFavoriteIds(updatedFavorites);
      }
      setDeleteTarget(null);
    } catch (deleteError) {
      // Retry re-opens the delete confirmation dialog for the same target rather
      // than deleting without a fresh confirmation.
      reportError(deleteError, () => setDeleteTarget(deleteTarget));
    } finally {
      setDeleteSubmitting(false);
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

        {error ? (
          <p className="app-notice-banner dashboard-error">
            {error.message}
            {error.retry ? (
              <button className="dashboard-error-retry" type="button" onClick={error.retry}>
                {formatUiMessage(locale, "dashboardActionRetry")}
              </button>
            ) : null}
          </p>
        ) : null}

        {loading ? (
          <DashboardSkeleton />
        ) : visibleWorkspaces.length === 0 ? (
          <section className="dashboard-empty-state">
            <h2>{activeTab === "favorites" ? formatUiMessage(locale, "dashboardEmptyFavoritesTitle") : formatUiMessage(locale, "dashboardEmptyAllTitle")}</h2>
            <p>{formatUiMessage(locale, "dashboardEmptyBody")}</p>
            <button className="app-button-primary" disabled={creatingWorkspace} type="button" onClick={() => void handleCreateWorkspace()}>
              {formatUiMessage(locale, "dashboardCreateFirst")}
            </button>
          </section>
        ) : viewMode === "card" ? (
          <section aria-label={formatUiMessage(locale, "dashboardViewGridLabel")} className="dashboard-grid" data-testid="notebook-list" role="grid">
            <DashboardCreateTile creating={creatingWorkspace} delay={visibleWorkspaces.length * 50} onCreate={() => void handleCreateWorkspace()} />
            {visibleWorkspaces.map((workspace, index) => {
              return (
                <WorkspaceListItem
                  key={workspace.id}
                  index={index}
                  mode="card"
                  onDelete={() => setDeleteTarget(workspace)}
                  onFavoriteToggle={() => void toggleFavorite(workspace.id)}
                  onRename={() => openRenameDialog(workspace)}
                  workspace={workspace}
                />
              );
            })}
          </section>
        ) : (
          <section className="dashboard-list-shell" role="table">
            <div className="dashboard-list-header" role="row">
              <div role="columnheader">{formatUiMessage(locale, "dashboardWorkspaceNameField")}</div>
              <div role="columnheader">{sourcesColumnLabel}</div>
              <div role="columnheader">{formatUiMessage(locale, "dashboardCreatedAtColumn")}</div>
              <div role="columnheader">{formatUiMessage(locale, "dashboardRoleColumn")}</div>
            </div>
            <ul aria-label={formatUiMessage(locale, "dashboardListLabel")} className="dashboard-list" data-testid="notebook-list">
              {visibleWorkspaces.map((workspace, index) => (
                <WorkspaceListItem
                  key={workspace.id}
                  index={index}
                  mode="list"
                  onDelete={() => setDeleteTarget(workspace)}
                  onFavoriteToggle={() => void toggleFavorite(workspace.id)}
                  onRename={() => openRenameDialog(workspace)}
                  workspace={workspace}
                />
              ))}
            </ul>
          </section>
        )}
      </section>

      {renameTarget ? (
        <div className="dashboard-modal-backdrop" onClick={dismissRenameDialog} role="presentation">
          <section
            aria-label={formatUiMessage(locale, "dashboardRenameDialogTitle")}
            aria-modal="true"
            className="dashboard-modal"
            role="dialog"
            onClick={(event) => event.stopPropagation()}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                dismissRenameDialog();
              }
            }}
          >
            <h2 className="dashboard-modal-title">{formatUiMessage(locale, "dashboardRenameDialogTitle")}</h2>
            <form
              className="dashboard-modal-form"
              onSubmit={(event) => {
                event.preventDefault();
                void submitRenameWorkspace();
              }}
            >
              <input
                aria-label={formatUiMessage(locale, "dashboardWorkspaceNameField")}
                autoComplete="off"
                autoFocus
                className="app-input"
                id="dashboard-rename-title"
                name="title"
                onChange={(event) => setRenameTitle(event.target.value)}
                value={renameTitle}
              />
              <div className="dashboard-modal-actions">
                <button className="dashboard-action-button" type="button" onClick={dismissRenameDialog}>
                  {formatUiMessage(locale, "commonCancel")}
                </button>
                <button className="app-button-primary" disabled={renameSubmitting} type="submit">
                  {formatUiMessage(locale, "dashboardRenameSubmit")}
                </button>
              </div>
            </form>
          </section>
        </div>
      ) : null}

      {deleteTarget ? (
        <div className="dashboard-modal-backdrop" onClick={() => setDeleteTarget(null)} role="presentation">
          <section
            aria-label={formatUiMessage(locale, "dashboardDeleteDialogTitle")}
            aria-modal="true"
            className="dashboard-modal"
            role="dialog"
            onClick={(event) => event.stopPropagation()}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                setDeleteTarget(null);
              }
            }}
          >
            <h2 className="dashboard-modal-title">{formatUiMessage(locale, "dashboardDeleteDialogTitle")}</h2>
            <p className="dashboard-modal-body">
              {formatUiMessage(locale, "dashboardDeleteDialogBody", {
                title: formatWorkspaceTitle(locale, deleteTarget),
              })}
            </p>
            <div className="dashboard-modal-actions">
              <button autoFocus className="dashboard-action-button" type="button" onClick={() => setDeleteTarget(null)}>
                {formatUiMessage(locale, "commonCancel")}
              </button>
              <button
                className="dashboard-button-danger"
                disabled={deleteSubmitting}
                type="button"
                onClick={() => void confirmDeleteWorkspace()}
              >
                {formatUiMessage(locale, "dashboardActionDelete")}
              </button>
            </div>
          </section>
        </div>
      ) : null}

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

      <div className="dashboard-main" style={{ paddingTop: 0 }}>
        <ProductChromeFooter />
      </div>
    </main>
  );
}