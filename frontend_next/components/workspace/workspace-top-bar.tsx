"use client";

import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";

import { ContextOsMark } from "../context-os-mark";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
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
  const auth = useAuth();
  const router = useRouter();
  const { locale, setLocale, setTheme, theme } = useUiPreferences();
  const [isTitleEditing, setIsTitleEditing] = useState(false);
  const [gearMenuOpen, setGearMenuOpen] = useState(false);
  const [avatarMenuOpen, setAvatarMenuOpen] = useState(false);
  const titleInputRef = useRef<HTMLInputElement | null>(null);
  const gearMenuRef = useRef<HTMLDivElement | null>(null);
  const avatarMenuRef = useRef<HTMLDivElement | null>(null);
  const safeWorkspaceTitle = workspaceTitle ?? "";
  const safeWorkspaceDescription = workspaceDescription ?? "";
  const currentUserLabel = auth.user?.full_name.trim() || auth.user?.email.trim() || formatUiMessage(locale, "workspaceAnonymousUser");
  const currentUserEmail = auth.user?.email.trim() || formatUiMessage(locale, "workspaceAnonymousUser");
  const newNotebookLabel = locale === "zh-CN" ? "新建笔记本" : "New Notebook";
  const displayTitle = safeWorkspaceTitle;

  useEffect(() => {
    if (!gearMenuOpen && !avatarMenuOpen) {
      return;
    }

    function closeMenus() {
      setGearMenuOpen(false);
      setAvatarMenuOpen(false);
    }

    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;

      if (gearMenuRef.current?.contains(target) || avatarMenuRef.current?.contains(target)) {
        return;
      }

      closeMenus();
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        closeMenus();
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [avatarMenuOpen, gearMenuOpen]);

  useEffect(() => {
    if (!isTitleEditing) {
      return;
    }

    titleInputRef.current?.focus();
    titleInputRef.current?.select();
  }, [isTitleEditing]);

  async function handleLogout() {
    setAvatarMenuOpen(false);
    await auth.logout();
    router.replace("/login");
  }

  function closeAllMenus() {
    setGearMenuOpen(false);
    setAvatarMenuOpen(false);
  }

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

  return (
    <header className={styles.topBar}>
      <div className={styles.topBarBrand}>
        <Link className={styles.topBarBrandBlock} href="/dashboard">
          <ContextOsMark className={styles.topBarMark} />
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
        <div className={styles.topBarActionGroup}>
          <button
            aria-label={formatUiMessage(locale, "workspaceCreateAction")}
            className={styles.topBarPrimaryButton}
            type="button"
            onClick={() => {
              closeAllMenus();
              onCreateWorkspaceSubmit();
            }}
          >
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeWidth="1.9" />
            </svg>
            <span className={styles.topBarActionLabel}>{newNotebookLabel}</span>
          </button>
          <Link className={styles.topBarActionButton} href={`/dashboard/${workspaceId}/share`}>
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M8 9.6 10.4 12 8 14.4 5.6 12 8 9.6Z" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M17 5.3 19.2 7.5 17 9.7 14.8 7.5 17 5.3Z" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M17 14.3 19.2 16.5 17 18.7 14.8 16.5 17 14.3Z" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M10 10.95 15.15 8.35M10 13.05l5.15 2.6" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
              <path d="M5.95 8.15 4.25 10.1v3.8l1.7 1.95" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
            </svg>
            <span className={styles.topBarActionLabel}>{formatUiMessage(locale, "workspaceDistribute")}</span>
          </Link>
          <Link className={styles.topBarActionButton} href={`/dashboard/${workspaceId}/api-access`}>
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path d="M8 8.5 4.5 12 8 15.5M16 8.5 19.5 12 16 15.5M13.5 6.5 10.5 17.5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
            </svg>
            <span className={styles.topBarActionLabel}>{formatUiMessage(locale, "workspaceApi")}</span>
          </Link>
        <div className={styles.menuAnchor} ref={gearMenuRef}>
          <button
            aria-expanded={gearMenuOpen}
            aria-haspopup="dialog"
            aria-label={formatUiMessage(locale, "dashboardSettingsLink")}
            className={styles.topBarActionButton}
            type="button"
            onClick={() => {
              setAvatarMenuOpen(false);
              setGearMenuOpen((current) => !current);
            }}
          >
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" viewBox="0 0 24 24">
              <path d="M4 21V14M4 10V3M12 21V8M12 4V3M20 21v-9M20 8V3M1 14h6M9 8h6M17 18h6" />
            </svg>
            <span>{formatUiMessage(locale, "dashboardSettingsLink")}</span>
          </button>

          {gearMenuOpen ? (
            <div className={`${styles.menuPanel} ${styles.menuPanelWide}`}>
              <section className={styles.menuSection}>
                <div className={styles.menuSectionTitle}>{formatUiMessage(locale, "workspaceMenuTheme")}</div>
                <div className={styles.menuChoiceGroup}>
                  {([
                    { value: "system" as const, label: formatUiMessage(locale, "workspaceThemeSystem") },
                    { value: "light" as const, label: formatUiMessage(locale, "workspaceThemeLight") },
                    { value: "dark" as const, label: formatUiMessage(locale, "workspaceThemeDark") },
                  ]).map((option) => (
                    <button
                      key={option.value}
                      aria-pressed={theme === option.value}
                      className={theme === option.value ? styles.menuChoiceActive : styles.menuChoice}
                      type="button"
                      onClick={() => {
                        setTheme(option.value);
                        setGearMenuOpen(false);
                      }}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </section>

              <section className={styles.menuSection}>
                <div className={styles.menuSectionTitle}>{formatUiMessage(locale, "workspaceMenuLanguage")}</div>
                <div className={styles.menuChoiceGroup}>
                  {([
                    { value: "zh-CN" as const, label: formatUiMessage(locale, "workspaceLanguageChinese") },
                    { value: "en" as const, label: formatUiMessage(locale, "workspaceLanguageEnglish") },
                  ]).map((option) => (
                    <button
                      key={option.value}
                      aria-pressed={locale === option.value}
                      className={locale === option.value ? styles.menuChoiceActive : styles.menuChoice}
                      type="button"
                      onClick={() => {
                        setLocale(option.value);
                        setGearMenuOpen(false);
                      }}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </section>
            </div>
          ) : null}
        </div>

        <div className={styles.menuAnchor} ref={avatarMenuRef}>
          <button
            aria-expanded={avatarMenuOpen}
            aria-haspopup="dialog"
            aria-label={formatUiMessage(locale, "workspaceOpenAccountMenu")}
            className={styles.topBarActionButton}
            type="button"
            onClick={() => {
              setGearMenuOpen(false);
              setAvatarMenuOpen((current) => !current);
            }}
          >
            <svg aria-hidden="true" className={styles.actionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                d="M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="2"
              />
              <path
                d="M4 20a8 8 0 0 1 16 0"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="2"
              />
            </svg>
            <span className={styles.topBarUserLabel}>{formatUiMessage(locale, "dashboardProfileLink")}</span>
          </button>

          {avatarMenuOpen ? (
            <div className={styles.menuPanel}>
              <div className={styles.menuAccountCard}>
                <div className={styles.menuAccountName}>{currentUserLabel}</div>
                <div className={styles.menuAccountEmail}>{currentUserEmail}</div>
              </div>
              <Link
                className={styles.menuChoice}
                href="/settings?tab=profile"
                onClick={() => {
                  setAvatarMenuOpen(false);
                }}
              >
                {formatUiMessage(locale, "dashboardProfileLink")}
              </Link>
              <button className={styles.menuDangerButton} type="button" onClick={() => void handleLogout()}>
                {formatUiMessage(locale, "workspaceLogout")}
              </button>
            </div>
          ) : null}
        </div>
        </div>
      </div>
    </header>
  );
}
