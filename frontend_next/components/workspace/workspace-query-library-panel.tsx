"use client";

import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { createPortal } from "react-dom";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import { searchQueryLibraryItems } from "../../lib/workspace/query-library/logic";
import { useQueryLibrary } from "../../lib/workspace/query-library/store";
import styles from "./workspace-shell.module.css";

type WorkspaceQueryLibraryPanelProps = {
  workspaceId: string;
  onInsert: (text: string) => boolean;
};

type HoverPreviewState = {
  text: string;
  top: number;
  left: number;
  maxWidth: number;
};

type HoveredAnchor = {
  element: HTMLElement;
  text: string;
};

function computeHoverPreview(anchor: HTMLElement, text: string): HoverPreviewState {
  const rect = anchor.getBoundingClientRect();
  const gap = 8;
  const maxWidth = Math.min(360, window.innerWidth * 0.4);
  let left = rect.right + gap;

  if (left + maxWidth > window.innerWidth - gap) {
    left = Math.max(gap, rect.left - maxWidth - gap);
  }

  const estimatedHeight = 120;
  const top = Math.max(gap, Math.min(rect.top, window.innerHeight - estimatedHeight - gap));

  return { text, top, left, maxWidth };
}

export function WorkspaceQueryLibraryPanel({ workspaceId, onInsert }: WorkspaceQueryLibraryPanelProps) {
  const { locale } = useUiPreferences();
  const { items, remove, touch, clear } = useQueryLibrary(workspaceId);
  const [searchQuery, setSearchQuery] = useState("");
  const [hoveredItem, setHoveredItem] = useState<HoverPreviewState | null>(null);
  const hideHoverTimeoutRef = useRef<number | null>(null);
  const hoveredAnchorRef = useRef<HoveredAnchor | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);

  const visibleItems = useMemo(
    () => searchQueryLibraryItems(items, searchQuery),
    [items, searchQuery],
  );

  function clearHideHoverTimeout() {
    if (hideHoverTimeoutRef.current !== null) {
      window.clearTimeout(hideHoverTimeoutRef.current);
      hideHoverTimeoutRef.current = null;
    }
  }

  function clearHoverPreview() {
    clearHideHoverTimeout();
    hoveredAnchorRef.current = null;
    setHoveredItem(null);
  }

  function showHoverPreview(event: MouseEvent<HTMLElement>, text: string) {
    clearHideHoverTimeout();
    hoveredAnchorRef.current = { element: event.currentTarget, text };
    setHoveredItem(computeHoverPreview(event.currentTarget, text));
  }

  function scheduleHideHoverPreview() {
    hideHoverTimeoutRef.current = window.setTimeout(() => {
      clearHoverPreview();
    }, 80);
  }

  function handleItemClick(id: string, text: string) {
    if (onInsert(text)) {
      touch(id);
    }
  }

  useEffect(() => {
    setSearchQuery("");
    clearHoverPreview();
  }, [workspaceId]);

  useEffect(() => {
    return () => {
      clearHideHoverTimeout();
    };
  }, []);

  useEffect(() => {
    if (!hoveredItem) {
      return;
    }

    function repositionHoverPreview() {
      const hovered = hoveredAnchorRef.current;
      if (!hovered) {
        return;
      }

      setHoveredItem(computeHoverPreview(hovered.element, hovered.text));
    }

    window.addEventListener("scroll", repositionHoverPreview, true);
    window.addEventListener("resize", repositionHoverPreview);

    const list = listRef.current;
    list?.addEventListener("scroll", repositionHoverPreview);

    return () => {
      window.removeEventListener("scroll", repositionHoverPreview, true);
      window.removeEventListener("resize", repositionHoverPreview);
      list?.removeEventListener("scroll", repositionHoverPreview);
    };
  }, [hoveredItem]);

  return (
    <section
      className={styles.queryLibrarySection}
      aria-label={formatUiMessage(locale, "workspaceQueryLibraryTitle")}
      data-testid="query-library-panel"
    >
      <div className={styles.queryLibraryHeader}>
        <h3 className={styles.queryLibraryTitle}>{formatUiMessage(locale, "workspaceQueryLibraryTitle")}</h3>
        {items.length > 0 ? (
          <button className={styles.queryLibraryClearButton} type="button" onClick={clear}>
            {formatUiMessage(locale, "workspaceQueryLibraryClear")}
          </button>
        ) : null}
      </div>

      <label className={styles.filterField}>
        <span className={styles.filterIcon} aria-hidden="true">
          <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="m21 21-4.35-4.35" strokeLinecap="round" strokeWidth="2" />
            <circle cx="11" cy="11" r="6" strokeWidth="2" />
          </svg>
        </span>
        <input
          type="search"
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
          placeholder={formatUiMessage(locale, "workspaceQueryLibrarySearchPlaceholder")}
          aria-label={formatUiMessage(locale, "workspaceQueryLibrarySearchPlaceholder")}
        />
      </label>

      <div className={styles.queryLibraryList} ref={listRef}>
        {items.length === 0 ? (
          <p className={styles.emptyState}>{formatUiMessage(locale, "workspaceQueryLibraryEmpty")}</p>
        ) : visibleItems.length === 0 ? (
          <p className={styles.emptyState}>{formatUiMessage(locale, "workspaceQueryLibraryNoMatch")}</p>
        ) : (
          visibleItems.map((item) => (
            <article key={item.id} className={styles.queryLibraryItem} data-testid="query-library-item">
              <button
                className={styles.queryLibraryItemButton}
                type="button"
                aria-label={formatUiMessage(locale, "workspaceQueryLibraryInsert")}
                onClick={() => handleItemClick(item.id, item.text)}
                onMouseEnter={(event) => showHoverPreview(event, item.text)}
                onMouseLeave={scheduleHideHoverPreview}
              >
                <span className={styles.queryLibraryItemText}>{item.text}</span>
              </button>
              <button
                className={styles.queryLibraryDeleteButton}
                type="button"
                aria-label={formatUiMessage(locale, "workspaceQueryLibraryDelete")}
                onClick={(event) => {
                  event.stopPropagation();
                  remove(item.id);
                }}
              >
                ×
              </button>
            </article>
          ))
        )}
      </div>

      {hoveredItem && typeof document !== "undefined"
        ? createPortal(
            <div
              className={styles.queryLibraryHoverPreview}
              style={{
                top: hoveredItem.top,
                left: hoveredItem.left,
                maxWidth: hoveredItem.maxWidth,
              }}
              onMouseEnter={clearHideHoverTimeout}
              onMouseLeave={scheduleHideHoverPreview}
            >
              {hoveredItem.text}
            </div>,
            document.body,
          )
        : null}
    </section>
  );
}
