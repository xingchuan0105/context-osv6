"use client";

import {
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  toggleCapability,
  type WorkspaceCapability,
} from "../../lib/workspace/capabilities";
import { IconSend, IconStop } from "./chat-icons";
import styles from "./workspace-chat.module.css";

const MIN_COMPOSER_TEXTAREA_HEIGHT = 52;
const AUTO_COMPOSER_TEXTAREA_MAX_HEIGHT = 192;
const MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT = 360;

const CAPABILITY_TOGGLES: Array<{
  id: WorkspaceCapability;
  testId: string;
  labelKey: "workspaceChatCapRag" | "workspaceChatCapSearch";
}> = [
  { id: "rag", testId: "workspace-chat-cap-rag", labelKey: "workspaceChatCapRag" },
  { id: "search", testId: "workspace-chat-cap-search", labelKey: "workspaceChatCapSearch" },
];

type ChatComposerProps = {
  draft: string;
  onDraftChange: (draft: string) => void;
  isStreaming: boolean;
  capabilities: WorkspaceCapability[];
  locale: "zh-CN" | "en";
  workspaceId: string;
  /** No sources selected: RAG chip disabled + hint (2026-07-18 product rule). */
  ragDisabled?: boolean;
  onSubmit: () => void;
  onStop?: () => void;
  onCapabilitiesChange: (next: WorkspaceCapability[]) => void;
  textareaRef?: React.RefObject<HTMLTextAreaElement | null>;
  onHeightChange?: (height: number) => void;
  /** Empty-thread hero content rendered above the composer (centered layout). */
  hero?: ReactNode;
};

export function ChatComposer({
  draft,
  onDraftChange,
  isStreaming,
  capabilities,
  locale,
  workspaceId,
  ragDisabled = false,
  onSubmit,
  onStop,
  onCapabilitiesChange,
  textareaRef: externalTextareaRef,
  onHeightChange,
  hero,
}: ChatComposerProps) {
  const internalTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const textareaRef = externalTextareaRef ?? internalTextareaRef;
  const composerCardRef = useRef<HTMLDivElement | null>(null);
  const composerResizeCleanupRef = useRef<(() => void) | null>(null);

  const [composerTextareaHeight, setComposerTextareaHeight] = useState<number | null>(null);
  const [isComposerResizing, setIsComposerResizing] = useState(false);

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

  // Cleanup resize on unmount
  useEffect(() => {
    return () => {
      composerResizeCleanupRef.current?.();
    };
  }, []);

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

  const handleToggleCapability = useCallback(
    (cap: WorkspaceCapability) => {
      if (cap === "rag" && ragDisabled) {
        return;
      }
      onCapabilitiesChange(toggleCapability(capabilities, cap));
      textareaRef.current?.focus();
    },
    [capabilities, onCapabilitiesChange, ragDisabled, textareaRef],
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

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
    }
  }

  function handleComposerResizeKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") {
      return;
    }

    const textarea = textareaRef.current;
    if (!textarea) {
      return;
    }

    event.preventDefault();
    const startingHeight = Math.max(
      Number.parseFloat(textarea.style.height) || 0,
      textarea.clientHeight,
      MIN_COMPOSER_TEXTAREA_HEIGHT,
    );
    const delta = event.key === "ArrowUp" ? 16 : -16;
    const nextHeight = Math.min(
      Math.max(startingHeight + delta, MIN_COMPOSER_TEXTAREA_HEIGHT),
      MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT,
    );
    setComposerTextareaHeight(nextHeight);
  }

  return (
    <div
      className={`${styles.composerCard}${hero ? ` ${styles.composerCardHero}` : ""}`}
      ref={composerCardRef}
    >
      {hero}
      <button
        aria-label={formatUiMessage(locale, "workspaceChatComposerResize")}
        aria-orientation="horizontal"
        aria-valuemax={MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT}
        aria-valuemin={MIN_COMPOSER_TEXTAREA_HEIGHT}
        aria-valuenow={Math.round(composerTextareaHeight ?? MIN_COMPOSER_TEXTAREA_HEIGHT)}
        className={`${styles.composerResizeHandle}${isComposerResizing ? ` ${styles.composerResizeHandleActive}` : ""}`}
        onKeyDown={handleComposerResizeKeyDown}
        onMouseDown={handleComposerResizeStart}
        role="separator"
        tabIndex={0}
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
            onDraftChange(event.target.value);
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
              className={styles.capabilityToggles}
              data-testid="workspace-chat-capability-toggles"
              role="group"
              aria-label={formatUiMessage(locale, "workspaceChatCapabilityLabel")}
            >
              {CAPABILITY_TOGGLES.map((cap) => {
                const pressed = capabilities.includes(cap.id);
                const disabled = cap.id === "rag" && ragDisabled;
                return (
                  <button
                    key={cap.id}
                    type="button"
                    className={`${styles.capTag}${pressed ? ` ${styles.capTagPressed}` : ""}`}
                    data-testid={cap.testId}
                    aria-pressed={pressed}
                    disabled={disabled}
                    title={
                      disabled
                        ? formatUiMessage(locale, "workspaceChatCapRagNeedsSources")
                        : undefined
                    }
                    onClick={() => handleToggleCapability(cap.id)}
                  >
                    {formatUiMessage(locale, cap.labelKey)}
                  </button>
                );
              })}
            </div>

            {ragDisabled ? (
              <p className={styles.hint} data-testid="workspace-chat-rag-needs-sources">
                {formatUiMessage(locale, "workspaceChatCapRagNeedsSources")}
              </p>
            ) : null}

            <p className={styles.hint}>{formatUiMessage(locale, "workspaceChatComposerHint")}</p>
          </div>

          {isStreaming ? (
            <button
              aria-label={formatUiMessage(locale, "workspaceChatStop")}
              className={styles.sendButton}
              data-testid="workspace-chat-stop"
              onClick={(event) => {
                event.preventDefault();
                onStop?.();
              }}
              type="button"
            >
              <IconStop className={styles.sendIcon} />
              <span className={styles.srOnly}>{formatUiMessage(locale, "workspaceChatStop")}</span>
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
