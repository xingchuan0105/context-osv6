"use client";

import { useEffect, useRef, useState } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { WorkspaceNoteSyncState, type WorkspaceNote } from "../../lib/workspace/model";
import { markdownToPlainText } from "./workspace-note-rich-text";
import { WorkspaceNoteEditorTiptap } from "./workspace-note-editor-tiptap";
import styles from "./workspace-right-rail.module.css";

export type WorkspaceNotesPaneProps = {
  activeNoteId: string | null;
  draftContent: string;
  draftTitle: string;
  loading: boolean;
  notes: WorkspaceNote[];
  error: string;
  syncState: WorkspaceNoteSyncState;
  onClearActiveNote: () => void;
  onCreateNote: () => void;
  onDeleteActiveNote: () => void;
  onDraftContentChange: (value: string) => void;
  onDraftTitleChange: (value: string) => void;
  onDeleteNote: (noteId: string) => void;
  onPromoteActiveNote: () => void;
  onPromoteNote: (noteId: string) => void;
  onSaveActiveNote: () => void;
  onSelectNote: (noteId: string) => void;
};

function deriveNoteTitleFromContent(content: string) {
  const plainText = markdownToPlainText(content).replace(/\s+/g, " ").trim();

  if (!plainText) {
    return "";
  }

  return plainText.length > 40 ? `${plainText.slice(0, 40).trimEnd()}...` : plainText;
}

export function WorkspaceNotesPane({
  activeNoteId,
  draftContent,
  draftTitle,
  loading: _loading,
  notes,
  error,
  syncState: _syncState,
  onClearActiveNote,
  onCreateNote,
  onDeleteActiveNote,
  onDraftContentChange,
  onDraftTitleChange,
  onDeleteNote,
  onPromoteActiveNote,
  onPromoteNote,
  onSaveActiveNote,
  onSelectNote,
}: WorkspaceNotesPaneProps) {
  const { locale } = useUiPreferences();
  const [openMenuNoteId, setOpenMenuNoteId] = useState<string | null>(null);
  const openMenuRef = useRef<HTMLDivElement | null>(null);
  const savedNotesLabel = locale === "zh-CN" ? "已保存笔记" : "Saved notes";
  const derivedDraftTitle = deriveNoteTitleFromContent(draftContent);
  const displayedDraftTitle = derivedDraftTitle || draftTitle || formatUiMessage(locale, "workspaceRightRail.untitledNote");

  useEffect(() => {
    if (!openMenuNoteId) {
      return;
    }

    function handlePointerDown(event: globalThis.MouseEvent) {
      const target = event.target as Node;

      if (openMenuRef.current?.contains(target)) {
        return;
      }

      setOpenMenuNoteId(null);
    }

    function handleKeyDown(event: globalThis.KeyboardEvent) {
      if (event.key === "Escape") {
        setOpenMenuNoteId(null);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [openMenuNoteId]);

  useEffect(() => {
    if (!activeNoteId) {
      return;
    }

    if (derivedDraftTitle !== draftTitle) {
      onDraftTitleChange(derivedDraftTitle);
    }
  }, [activeNoteId, derivedDraftTitle, draftTitle, onDraftTitleChange]);

  if (activeNoteId) {
    return (
      <section
        className={`${styles.pane} ${styles.editorPane}`}
        aria-label={formatUiMessage(locale, "workspaceRightRail.notesSectionTitle")}
      >
        <div className={styles.editorTopBar}>
          <div className={styles.paneHeading}>
            <h2 className={styles.paneTitle}>{displayedDraftTitle}</h2>
          </div>
          <button className={styles.closeButton} onClick={onClearActiveNote} type="button">
            {formatUiMessage(locale, "workspaceRightRail.closeViewerAction")}
          </button>
        </div>

        {error ? <div className={styles.error}>{error}</div> : null}

        <div className={`${styles.editorField} ${styles.editorBodyField}`}>
          <WorkspaceNoteEditorTiptap
            contentLabel={formatUiMessage(locale, "workspaceRightRail.noteContentLabel")}
            locale={locale}
            onChange={onDraftContentChange}
            placeholder={locale === "zh-CN" ? "开始记录笔记…" : "Start writing notes…"}
            toolbarLabel={formatUiMessage(locale, "workspaceRightRail.noteEditorToolbar")}
            value={draftContent}
          />
        </div>

        <div className={styles.editorFooter}>
          <div className={styles.buttonRow}>
            <button className={`${styles.button} ${styles.buttonPrimary}`} onClick={onSaveActiveNote} type="button">
              {formatUiMessage(locale, "workspaceRightRail.saveNoteAction")}
            </button>
            <button className={styles.button} onClick={onPromoteActiveNote} type="button">
              {formatUiMessage(locale, "workspaceRightRail.promoteNoteAction")}
            </button>
            <button className={`${styles.button} ${styles.buttonDanger}`} onClick={onDeleteActiveNote} type="button">
              {formatUiMessage(locale, "workspaceRightRail.deleteNoteAction")}
            </button>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className={`${styles.pane} ${styles.railPane}`} aria-label={formatUiMessage(locale, "workspaceRightRail.notesSectionTitle")}>
      <div className={styles.paneHeader}>
        <div className={styles.paneHeading}>
          <div className={styles.paneTitleRow}>
            <h2 className={styles.paneTitle}>{formatUiMessage(locale, "workspaceRightRail.notesSectionTitle")}</h2>
          </div>
        </div>
      </div>

      <div className={styles.sectionControls}>
        <button className={`${styles.sectionActionButton} ${styles.sectionActionButtonDark}`} onClick={onCreateNote} type="button">
          <svg aria-hidden="true" className={styles.sectionActionIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M12 5v14M5 12h14" strokeLinecap="round" strokeWidth="1.8" />
          </svg>
          <span>{formatUiMessage(locale, "workspaceRightRail.newNoteAction")}</span>
        </button>

        {error ? <div className={styles.error}>{error}</div> : null}
      </div>

      <div className={`${styles.sectionScroller} ${styles.notesSectionScroller}`}>
        {notes.length > 0 ? (
          <div className={styles.notesGroup}>
            <p className={styles.sectionListLabel}>{savedNotesLabel}</p>
            <ul aria-label={formatUiMessage(locale, "workspaceRightRail.notesListLabel")} className={styles.list}>
              {notes.map((note) => {
                const active = note.id === activeNoteId;
                const menuOpen = note.id === openMenuNoteId;
                const noteTitle = note.title || formatUiMessage(locale, "workspaceRightRail.untitledNote");
                const notePreview =
                  markdownToPlainText(note.preview || note.content) || formatUiMessage(locale, "workspaceRightRail.emptyNotePreview");

                return (
                  <li
                    className={`${styles.listItem} ${styles.noteListItem}${active ? ` ${styles.listItemFocused}` : ""}`}
                    key={note.id}
                  >
                    <div className={styles.listItemTopRow}>
                      <button
                        aria-label={`${noteTitle} ${notePreview}`}
                        className={styles.noteSelectButton}
                        onClick={() => onSelectNote(note.id)}
                        type="button"
                      >
                        <span className={styles.noteTitleRow}>
                          <span className={styles.noteTitleText}>{noteTitle}</span>
                        </span>
                        <span className={styles.listItemSubtitle}>{notePreview}</span>
                        {note.promoted_document_id ? (
                          <span className={styles.listItemMetaRow}>
                            <span className={styles.inlineBadge}>
                              {formatUiMessage(locale, "workspaceRightRail.promotedNoteBadge")}
                            </span>
                          </span>
                        ) : null}
                      </button>

                      <div className={styles.itemMenuAnchor} ref={menuOpen ? openMenuRef : null}>
                        <button
                          aria-expanded={menuOpen}
                          aria-haspopup="menu"
                          aria-label={`Note actions for ${noteTitle}`}
                          className={styles.itemMenuTrigger}
                          type="button"
                          onClick={() => setOpenMenuNoteId((current) => (current === note.id ? null : note.id))}
                        >
                          <svg aria-hidden="true" className={styles.itemMenuIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path d="M12 6.75a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5ZM12 13.25a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5ZM12 19.75a1.25 1.25 0 1 1 0-2.5 1.25 1.25 0 0 1 0 2.5Z" fill="currentColor" stroke="none" />
                          </svg>
                        </button>

                        {menuOpen ? (
                          <div className={styles.itemMenu} role="menu">
                            <button
                              className={styles.itemMenuButton}
                              role="menuitem"
                              type="button"
                              onClick={() => {
                                setOpenMenuNoteId(null);
                                onPromoteNote(note.id);
                              }}
                            >
                              {formatUiMessage(locale, "workspaceRightRail.promoteNoteAction")}
                            </button>
                            <button
                              className={`${styles.itemMenuButton} ${styles.itemMenuButtonDanger}`}
                              role="menuitem"
                              type="button"
                              onClick={() => {
                                setOpenMenuNoteId(null);
                                onDeleteNote(note.id);
                              }}
                            >
                              {formatUiMessage(locale, "workspaceRightRail.deleteNoteAction")}
                            </button>
                          </div>
                        ) : null}
                      </div>
                    </div>
                  </li>
                );
              })}
            </ul>
          </div>
        ) : (
          <div className={`${styles.emptyStateBlock} ${styles.notesEmptyStateBlock}`}>
            <div className={`${styles.emptyStateTitle} ${styles.notesEmptyStateTitle}`}>
              {formatUiMessage(locale, "workspaceRightRail.emptyNotesTitle")}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
