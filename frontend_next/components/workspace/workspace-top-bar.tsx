"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";

import { DesktopSettingsDrawer } from "../desktop/DesktopSettingsDrawer";
import { DesktopStatusBadge } from "../desktop/DesktopStatusBadge";
import { ContextOsMark } from "../context-os-mark";
import { AccountMenu } from "../account-menu";
import { PlanEntry } from "../plan-entry";
import { WorkspaceApiAccessModal } from "../api-access/workspace-api-access-modal";
import { WorkspaceShareQuickModal } from "../share/workspace-share-quick-modal";
import { formatUiMessage } from "../../lib/i18n/messages";
import { isTauri } from "../../lib/runtime/tauri-ipc";
import { useUiPreferences } from "../../lib/ui-preferences";
import styles from "./workspace-shell.module.css";

type WorkspaceTopBarProps = {
  workspaceId: string;
  workspaceTitle: string;
  workspaceDescription: string;
  workspaceTitleDraft: string;
  onWorkspaceTitleDraftChange: (value: string) => void;
  onSaveWorkspaceTitle: () => void;
  onCreateWorkspaceSubmit: () => void;
};

export function WorkspaceTopBar({
  workspaceId,
  workspaceTitle,
  workspaceDescription,
  workspaceTitleDraft,
  onWorkspaceTitleDraftChange,
  onSaveWorkspaceTitle,
  onCreateWorkspaceSubmit,
}: WorkspaceTopBarProps) {
  const { locale } = useUiPreferences();
  const [isTitleEditing, setIsTitleEditing] = useState(false);
  const [desktopDrawerOpen, setDesktopDrawerOpen] = useState(false);
  const [shareOpen, setShareOpen] = useState(false);
  const [apiOpen, setApiOpen] = useState(false);
  const desktopRuntime = isTauri();
  const titleInputRef = useRef<HTMLInputElement | null>(null);
  const safeWorkspaceTitle = workspaceTitle ?? "";
  const safeWorkspaceDescription = workspaceDescription ?? "";
  const newWorkspaceLabel = formatUiMessage(locale, "workspaceCreateDialogLabel");
  const displayTitle = safeWorkspaceTitle;

  useEffect(() => {
    if (!isTitleEditing) {
      return;
    }

    titleInputRef.current?.focus();
    titleInputRef.current?.select();
  }, [isTitleEditing]);

  function saveWorkspaceTitle() {
    setIsTitleEditing(false);
    if (!workspaceTitleDraft.trim()) {
      onWorkspaceTitleDraftChange(workspaceTitle);
      return;
    }
    onSaveWorkspaceTitle();
  }

  function cancelWorkspaceTitleEdit() {
    setIsTitleEditing(false);
    onWorkspaceTitleDraftChange(workspaceTitle);
  }

  // IA: primary top-bar actions stay product-only (workspace, share, API).
  // No marketing chrome (pricing / blog / family dump). Desktop discoverability
  // for web users is secondary: avatar menu + ProductChromeFooter + /help.
  return (
    <header className={styles.topBar} data-testid="workspace-top-bar" data-marketing-chrome="false">
      <div className={styles.topBarBrand}>
        <Link className={styles.topBarBrandBlock} href="/dashboard">
          <ContextOsMark size={32} className={styles.topBarMark} />
          <span className={styles.topBarBrandName}>Context-OS</span>
        </Link>

        <div className={styles.topBarDivider} aria-hidden="true" />

        <div className={styles.topBarTitleArea}>
          <form
            className={styles.titleForm}
            onSubmit={(event) => {
              event.preventDefault();
              saveWorkspaceTitle();
            }}
          >
            <label className={styles.srOnly} htmlFor="workspace-title">
              {formatUiMessage(locale, "workspaceSessionTitleField")}
            </label>
            {isTitleEditing ? (
              <input
                id="workspace-title"
                ref={titleInputRef}
                aria-label={formatUiMessage(locale, "workspaceSessionTitleField")}
                className={styles.titleInput}
                value={workspaceTitleDraft}
                onBlur={saveWorkspaceTitle}
                onChange={(event) => onWorkspaceTitleDraftChange(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    saveWorkspaceTitle();
                  }

                  if (event.key === "Escape") {
                    event.preventDefault();
                    cancelWorkspaceTitleEdit();
                  }
                }}
              />
            ) : (
              <button
                id="workspace-title"
                className={styles.topBarTitleTrigger}
                type="button"
                title={safeWorkspaceDescription.trim() || safeWorkspaceTitle}
                onClick={() => {
                  onWorkspaceTitleDraftChange(safeWorkspaceTitle);
                  setIsTitleEditing(true);
                }}
              >
                {displayTitle}
              </button>
            )}
          </form>
        </div>
      </div>

      <div className={styles.topBarActions}>
        <PlanEntry locale={locale} size="compact" />
        {desktopRuntime ? (
          <>
            <DesktopStatusBadge onClick={() => setDesktopDrawerOpen(true)} />
            <button
              aria-label="客户端设置"
              className={styles.topBarActionButton}
              type="button"
              onClick={() => {
                setDesktopDrawerOpen(true);
              }}
            >
              <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path
                  d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth="1.8"
                />
                <path
                  d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.05.05a2.1 2.1 0 0 1-2.96 2.96l-.05-.05a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1 1.51V21a2.1 2.1 0 0 1-4.2 0v-.09a1.7 1.7 0 0 0-1-1.51 1.7 1.7 0 0 0-1.87.34l-.05.05a2.1 2.1 0 0 1-2.96-2.96l.05-.05A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.51-1H3a2.1 2.1 0 0 1 0-4.2h.09a1.7 1.7 0 0 0 1.51-1 1.7 1.7 0 0 0-.34-1.87l-.05-.05a2.1 2.1 0 0 1 2.96-2.96l.05.05a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1-1.51V3a2.1 2.1 0 0 1 4.2 0v.09a1.7 1.7 0 0 0 1 1.51 1.7 1.7 0 0 0 1.87-.34l.05-.05a2.1 2.1 0 0 1 2.96 2.96l-.05.05a1.7 1.7 0 0 0-.34 1.87V9a1.7 1.7 0 0 0 1.51 1H21a2.1 2.1 0 0 1 0 4.2h-.09a1.7 1.7 0 0 0-1.51 1Z"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth="1.8"
                />
              </svg>
              <span className={styles.topBarActionLabel}>客户端设置</span>
            </button>
          </>
        ) : null}
        <div className={styles.topBarActionGroup}>
          <button
            aria-label={formatUiMessage(locale, "workspaceCreateAction")}
            className={`${styles.topBarPrimaryButton} app-button-create`}
            type="button"
            onClick={() => {
              onCreateWorkspaceSubmit();
            }}
          >
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeWidth="1.8" />
            </svg>
            <span className={styles.topBarActionLabel}>{newWorkspaceLabel}</span>
          </button>
          <button
            className={styles.topBarActionButton}
            data-testid="workspace-topbar-share"
            type="button"
            onClick={() => setShareOpen(true)}
          >
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M8 9.6 10.4 12 8 14.4 5.6 12 8 9.6Z" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M17 5.3 19.2 7.5 17 9.7 14.8 7.5 17 5.3Z" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M17 14.3 19.2 16.5 17 18.7 14.8 16.5 17 14.3Z" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M10 10.95 15.15 8.35M10 13.05l5.15 2.6" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M5.95 8.15 4.25 10.1v3.8l1.7 1.95" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
            </svg>
            <span className={styles.topBarActionLabel}>{formatUiMessage(locale, "workspaceDistribute")}</span>
          </button>
          <button
            className={styles.topBarActionButton}
            data-testid="workspace-topbar-api"
            type="button"
            onClick={() => setApiOpen(true)}
          >
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M8 8.5 4.5 12 8 15.5M16 8.5 19.5 12 16 15.5M13.5 6.5 10.5 17.5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
            </svg>
            <span className={styles.topBarActionLabel}>{formatUiMessage(locale, "workspaceApi")}</span>
          </button>
        </div>
        <AccountMenu locale={locale} />
      </div>
      {desktopRuntime ? (
        <DesktopSettingsDrawer open={desktopDrawerOpen} onClose={() => setDesktopDrawerOpen(false)} />
      ) : null}
      <WorkspaceShareQuickModal
        open={shareOpen}
        workspaceId={workspaceId}
        onClose={() => setShareOpen(false)}
      />
      <WorkspaceApiAccessModal
        open={apiOpen}
        workspaceId={workspaceId}
        onClose={() => setApiOpen(false)}
      />
    </header>
  );
}
