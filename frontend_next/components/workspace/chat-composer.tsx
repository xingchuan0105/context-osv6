"use client";

import {
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { formatUiMessage } from "../../lib/i18n/messages";
import { type WorkspaceChatMode } from "../../lib/workspace/ui-store";
import { IconChevronUp, IconSend, IconStop } from "./chat-icons";
import styles from "./workspace-chat.module.css";

const CHAT_MODE_ORDER: WorkspaceChatMode[] = ["rag", "search", "chat", "write"];
const MIN_COMPOSER_TEXTAREA_HEIGHT = 52;
const AUTO_COMPOSER_TEXTAREA_MAX_HEIGHT = 192;
const MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT = 360;
/** U14: brief open delay avoids flicker when the pointer grazes the trigger. */
const MODE_MENU_OPEN_DELAY_MS = 60;
/** U14: leave delay bridges the gap between trigger and floating menu. */
const MODE_MENU_CLOSE_DELAY_MS = 200;

function getModeLabel(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return formatUiMessage(locale, "workspaceChatModeRag");
    case "search":
      return formatUiMessage(locale, "workspaceChatModeSearch");
    case "write":
      return formatUiMessage(locale, "workspaceChatModeWrite");
    case "chat":
    default:
      return formatUiMessage(locale, "workspaceChatModeChat");
  }
}

function getModeCode(mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return "RAG";
    case "search":
      return "web_search";
    case "write":
      return "write";
    case "chat":
    default:
      return "chat";
  }
}

function getModeHint(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return formatUiMessage(locale, "workspaceChatModeHintRag");
    case "search":
      return formatUiMessage(locale, "workspaceChatModeHintSearch");
    case "write":
      return formatUiMessage(locale, "workspaceChatModeHintWrite");
    case "chat":
    default:
      return formatUiMessage(locale, "workspaceChatModeHintChat");
  }
}

function getModeIndex(mode: WorkspaceChatMode) {
  return Math.max(CHAT_MODE_ORDER.indexOf(mode), 0);
}

function canUseHoverOpen() {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(hover: hover)").matches
  );
}

type ChatComposerProps = {
  draft: string;
  onDraftChange: (draft: string) => void;
  isStreaming: boolean;
  effectiveChatMode: WorkspaceChatMode;
  locale: "zh-CN" | "en";
  workspaceId: string;
  onSubmit: () => void;
  onStop?: () => void;
  onModeChange: (mode: WorkspaceChatMode) => void;
  textareaRef?: React.RefObject<HTMLTextAreaElement | null>;
  onHeightChange?: (height: number) => void;
};

export function ChatComposer({
  draft,
  onDraftChange,
  isStreaming,
  effectiveChatMode,
  locale,
  workspaceId,
  onSubmit,
  onStop,
  onModeChange,
  textareaRef: externalTextareaRef,
  onHeightChange,
}: ChatComposerProps) {
  const internalTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const textareaRef = externalTextareaRef ?? internalTextareaRef;
  const composerCardRef = useRef<HTMLDivElement | null>(null);
  const composerResizeCleanupRef = useRef<(() => void) | null>(null);
  const modeMenuRef = useRef<HTMLDivElement | null>(null);
  const modeMenuOpenTimerRef = useRef<number | null>(null);
  const modeMenuCloseTimerRef = useRef<number | null>(null);

  const [showModeMenu, setShowModeMenu] = useState(false);
  const [modeMenuActiveIndex, setModeMenuActiveIndex] = useState(0);
  const [composerTextareaHeight, setComposerTextareaHeight] = useState<number | null>(null);
  const [isComposerResizing, setIsComposerResizing] = useState(false);

  const activeModeLabel = getModeLabel(locale, effectiveChatMode);

  const clearModeMenuTimers = useCallback(() => {
    if (modeMenuOpenTimerRef.current != null) {
      window.clearTimeout(modeMenuOpenTimerRef.current);
      modeMenuOpenTimerRef.current = null;
    }
    if (modeMenuCloseTimerRef.current != null) {
      window.clearTimeout(modeMenuCloseTimerRef.current);
      modeMenuCloseTimerRef.current = null;
    }
  }, []);

  const openModeMenu = useCallback(() => {
    setModeMenuActiveIndex(getModeIndex(effectiveChatMode));
    setShowModeMenu(true);
  }, [effectiveChatMode]);

  const closeModeMenu = useCallback(() => {
    clearModeMenuTimers();
    setShowModeMenu(false);
  }, [clearModeMenuTimers]);

  const scheduleOpenModeMenu = useCallback(() => {
    if (!canUseHoverOpen()) {
      return;
    }
    if (modeMenuCloseTimerRef.current != null) {
      window.clearTimeout(modeMenuCloseTimerRef.current);
      modeMenuCloseTimerRef.current = null;
    }
    if (showModeMenu) {
      return;
    }
    if (modeMenuOpenTimerRef.current != null) {
      return;
    }
    modeMenuOpenTimerRef.current = window.setTimeout(() => {
      modeMenuOpenTimerRef.current = null;
      openModeMenu();
    }, MODE_MENU_OPEN_DELAY_MS);
  }, [openModeMenu, showModeMenu]);

  const scheduleCloseModeMenu = useCallback(() => {
    if (!canUseHoverOpen()) {
      return;
    }
    if (modeMenuOpenTimerRef.current != null) {
      window.clearTimeout(modeMenuOpenTimerRef.current);
      modeMenuOpenTimerRef.current = null;
    }
    if (modeMenuCloseTimerRef.current != null) {
      return;
    }
    modeMenuCloseTimerRef.current = window.setTimeout(() => {
      modeMenuCloseTimerRef.current = null;
      setShowModeMenu(false);
    }, MODE_MENU_CLOSE_DELAY_MS);
  }, []);

  // Auto-resize textarea
  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }

    textarea.style.height = "0px";
    const contentHeight = Math.max(textarea.scrollHeight, MIN_COMPOSER_TEXTAREA_HEIGHT);
    const nextTextareaHeight =
      composerTextareaHeight === null
        ? Math.min(contentHeight, AUTO_COMPOSER_TEXTAREA_MAX_HEIGHT)
        : Math.min(
            Math.max(contentHeight, composerTextareaHeight),
            MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT,
          );
    textarea.style.height = `${nextTextareaHeight}px`;
  }, [composerTextareaHeight, draft, textareaRef]);

  // Close mode menu on outside click (touch / pointer)
  useEffect(() => {
    if (!showModeMenu) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      if (!modeMenuRef.current?.contains(event.target as Node)) {
        closeModeMenu();
      }
    }

    window.addEventListener("mousedown", handlePointerDown);

    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
    };
  }, [showModeMenu, closeModeMenu]);

  // Cleanup timers + resize on unmount
  useEffect(() => {
    return () => {
      clearModeMenuTimers();
      composerResizeCleanupRef.current?.();
    };
  }, [clearModeMenuTimers]);

  // Report height changes to parent for shell clearance
  useEffect(() => {
    const composerCard = composerCardRef.current;
    if (!composerCard || !onHeightChange) {
      return;
    }

    function reportHeight() {
      const height = Math.ceil(composerCard!.getBoundingClientRect().height);
      if (height > 0) {
        onHeightChange!(height);
      }
    }

    reportHeight();
    window.addEventListener("resize", reportHeight);

    if (typeof ResizeObserver === "undefined") {
      return () => {
        window.removeEventListener("resize", reportHeight);
      };
    }

    const observer = new ResizeObserver(() => {
      reportHeight();
    });
    observer.observe(composerCard);

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", reportHeight);
    };
  }, [onHeightChange]);

  const applyModeSelection = useCallback(
    (mode: WorkspaceChatMode) => {
      onModeChange(mode);
      setModeMenuActiveIndex(getModeIndex(mode));
      closeModeMenu();
      textareaRef.current?.focus();
    },
    [closeModeMenu, onModeChange, textareaRef],
  );

  function handleComposerResizeStart(event: ReactMouseEvent<HTMLButtonElement>) {
    if (event.button !== 0) {
      return;
    }

    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }

    event.preventDefault();
    composerResizeCleanupRef.current?.();

    const startingHeight = Math.max(
      Number.parseFloat(textarea.style.height) || 0,
      textarea.clientHeight,
      MIN_COMPOSER_TEXTAREA_HEIGHT,
    );
    const startY = event.clientY;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    setIsComposerResizing(true);
    document.body.style.cursor = "ns-resize";
    document.body.style.userSelect = "none";

    function handleMouseMove(moveEvent: MouseEvent) {
      const nextHeight = Math.min(
        Math.max(startingHeight + (startY - moveEvent.clientY), MIN_COMPOSER_TEXTAREA_HEIGHT),
        MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT,
      );
      setComposerTextareaHeight(nextHeight);
    }

    function cleanup() {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      setIsComposerResizing(false);
      composerResizeCleanupRef.current = null;
    }

    function handleMouseUp() {
      cleanup();
    }

    composerResizeCleanupRef.current = cleanup;
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function handleModeTriggerKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!showModeMenu) {
        openModeMenu();
      }
      setModeMenuActiveIndex((current) => {
        if (!showModeMenu) {
          return getModeIndex(effectiveChatMode);
        }
        if (event.key === "ArrowDown") {
          return (current + 1) % CHAT_MODE_ORDER.length;
        }
        return (current - 1 + CHAT_MODE_ORDER.length) % CHAT_MODE_ORDER.length;
      });
      return;
    }

    if (event.key === "Escape" && showModeMenu) {
      event.preventDefault();
      closeModeMenu();
      return;
    }

    if (event.key === "Enter" || event.key === " ") {
      // Keep native button activation for open/toggle via click handler.
      if (showModeMenu && event.key === "Enter") {
        event.preventDefault();
        applyModeSelection(CHAT_MODE_ORDER[modeMenuActiveIndex] ?? effectiveChatMode);
      }
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (showModeMenu) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeModeMenu();
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setModeMenuActiveIndex((current) => (current + 1) % CHAT_MODE_ORDER.length);
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        setModeMenuActiveIndex(
          (current) => (current - 1 + CHAT_MODE_ORDER.length) % CHAT_MODE_ORDER.length,
        );
        return;
      }

      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        applyModeSelection(CHAT_MODE_ORDER[modeMenuActiveIndex] ?? effectiveChatMode);
        return;
      }
    }

    if (
      event.key === "/" &&
      !event.shiftKey &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      draft.trim().length === 0
    ) {
      event.preventDefault();
      openModeMenu();
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
    }
  }

  return (
    <div className={styles.composerCard} ref={composerCardRef}>
      <button
        aria-label={locale === "zh-CN" ? "调整输入框高度" : "Resize composer"}
        className={`${styles.composerResizeHandle}${isComposerResizing ? ` ${styles.composerResizeHandleActive}` : ""}`}
        onMouseDown={handleComposerResizeStart}
        type="button"
      >
        <span className={styles.composerResizeGrip} aria-hidden="true" />
      </button>

      <form
        className={styles.composerForm}
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit();
        }}
      >
        <label className={styles.srOnly} htmlFor={`workspace-chat-composer-${workspaceId}`}>
          {formatUiMessage(locale, "workspaceChatComposerLabel")}
        </label>

        <textarea
          className={styles.textarea}
          data-testid="workspace-chat-composer"
          disabled={isStreaming}
          id={`workspace-chat-composer-${workspaceId}`}
          onChange={(event) => {
            const nextDraft = event.target.value;
            onDraftChange(nextDraft);

            if (showModeMenu && nextDraft.trim().length > 0) {
              closeModeMenu();
            }
          }}
          onKeyDown={handleKeyDown}
          placeholder={formatUiMessage(locale, "workspaceChatComposerPlaceholder")}
          ref={textareaRef}
          rows={1}
          value={draft}
        />

        <div className={styles.composerToolbar}>
          <div className={styles.toolbarLeft}>
            <div
              className={styles.modeMenuAnchor}
              data-testid="workspace-chat-mode-anchor"
              ref={modeMenuRef}
              onMouseEnter={scheduleOpenModeMenu}
              onMouseLeave={scheduleCloseModeMenu}
            >
              <button
                aria-expanded={showModeMenu}
                aria-haspopup="menu"
                aria-label={formatUiMessage(locale, "workspaceChatModeLabel")}
                className={`${styles.modeTag}${showModeMenu ? ` ${styles.modeTagOpen}` : ""}`}
                data-testid="workspace-chat-mode-button"
                onClick={() => {
                  clearModeMenuTimers();
                  if (showModeMenu) {
                    setShowModeMenu(false);
                    return;
                  }
                  openModeMenu();
                }}
                onKeyDown={handleModeTriggerKeyDown}
                type="button"
              >
                <span>{activeModeLabel}</span>
                <IconChevronUp className={styles.modeTagChevron} />
              </button>

              {showModeMenu ? (
                <div
                  className={styles.modeMenu}
                  data-testid="workspace-chat-mode-menu"
                  role="menu"
                  aria-label={formatUiMessage(locale, "workspaceChatModeLabel")}
                >
                  {CHAT_MODE_ORDER.map((mode, index) => {
                    const selected = mode === effectiveChatMode;
                    const active = index === modeMenuActiveIndex;
                    return (
                      <button
                        aria-checked={selected}
                        className={[
                          styles.modeMenuItem,
                          selected ? styles.modeMenuItemSelected : "",
                          active ? styles.modeMenuItemActive : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        data-testid={`workspace-chat-mode-${mode}`}
                        key={mode}
                        onClick={() => applyModeSelection(mode)}
                        role="menuitemradio"
                        type="button"
                      >
                        <span className={styles.modeMenuItemText}>
                          <span className={styles.modeMenuItemLabelRow}>
                            {selected ? (
                              <span aria-hidden="true" className={styles.modeMenuItemCheck}>
                                ✓
                              </span>
                            ) : (
                              <span aria-hidden="true" className={styles.modeMenuItemCheckSpacer} />
                            )}
                            <span className={styles.modeMenuItemLabel}>
                              {getModeLabel(locale, mode)}
                            </span>
                          </span>
                          <span className={styles.modeMenuItemHint}>{getModeHint(locale, mode)}</span>
                        </span>
                        <span className={styles.modeMenuItemCode}>{getModeCode(mode)}</span>
                      </button>
                    );
                  })}
                </div>
              ) : null}
            </div>

            {effectiveChatMode === "write" ? (
              <p
                className={styles.writeUsageHint}
                data-testid="workspace-chat-write-usage-hint"
                role="note"
              >
                {formatUiMessage(locale, "workspaceChatModeWriteUsageHint")}
              </p>
            ) : (
              <p className={styles.hint}>{formatUiMessage(locale, "workspaceChatComposerHint")}</p>
            )}
          </div>

          {isStreaming ? (
            <button
              aria-label={locale === "zh-CN" ? "停止" : "Stop"}
              className={styles.sendButton}
              data-testid="workspace-chat-stop"
              onClick={(event) => {
                event.preventDefault();
                onStop?.();
              }}
              type="button"
            >
              <IconStop className={styles.sendIcon} />
              <span className={styles.srOnly}>{locale === "zh-CN" ? "停止" : "Stop"}</span>
            </button>
          ) : (
            <button
              aria-label={formatUiMessage(locale, "workspaceSend")}
              className={styles.sendButton}
              data-testid="workspace-chat-send"
              disabled={draft.trim().length === 0}
              type="submit"
            >
              <IconSend className={styles.sendIcon} />
              <span className={styles.srOnly}>{formatUiMessage(locale, "workspaceSend")}</span>
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
