"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";

import { DesktopSettingsDrawer } from "../desktop/DesktopSettingsDrawer";
import { ContextOsMark } from "../context-os-mark";
import { AccountMenu } from "../account-menu";
import { NotificationBell } from "../notifications/notification-bell";
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

  // IA (PRODUCT_IA §5): 分享 is the T0 top-bar action — one capsule, one
  // behavior: opens the share modal directly; 访问/API fold into the modal.
  // No 工作台|设置 wayfinding (brand returns home, settings lives in the
  // account menu), no 客户端 capsule (dashboard toolbar hosts client discovery).
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
        {desktopRuntime ? (
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
          {/* 分享 is T0: one capsule, one behavior — opens the share modal
              directly. 访问/API fold into the share modal (PRODUCT_IA §5). */}
          <button
            className={styles.shareButton}
            data-testid="workspace-topbar-share"
            type="button"
            onClick={() => setShareOpen(true)}
          >
            {formatUiMessage(locale, "workspaceDistribute")}
          </button>
        </div>
        <NotificationBell locale={locale} />
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
    </header>
  );
}
