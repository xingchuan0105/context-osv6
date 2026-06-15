"use client";

import Link from "next/link";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { type DashboardLocale } from "../../../lib/dashboard/model";
import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";
import {
  type DashboardViewMode,
  type DashboardWorkspaceItem,
  formatWorkspaceSourceCount,
  formatWorkspaceTitle,
  getWorkspaceGlyph,
  getWorkspaceTone,
} from "./dashboard-utils";

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

export function DashboardCreateTile({
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

export function WorkspaceListItem({
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
        <article className="dashboard-workspace-card" data-testid="dashboard-workspace-item" data-workspace-id={workspace.id}>
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
      <article className="dashboard-workspace-card" data-testid="dashboard-workspace-item" data-workspace-id={workspace.id}>
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