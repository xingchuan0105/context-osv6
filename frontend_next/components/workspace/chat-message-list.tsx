"use client";

import { Fragment, type ReactNode, useEffect, useRef, useState } from "react";
import { formatUiMessage } from "../../lib/i18n/messages";
import type {
  WorkspaceCitationRequest,
  WorkspaceWebSourcesRequest,
} from "../../lib/workspace/model";
import type { WorkspaceCapability } from "../../lib/workspace/capabilities";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import {
  type AnswerBlock,
  type Citation,
} from "../../lib/workspace/stream";
import styles from "./workspace-chat.module.css";
import type { ProgressEntry, UiChatMessage, UiProgressSnapshot } from "../../hooks/use-chat-session";
import {
  IconCheck,
  IconCopy,
  IconEdit,
  IconNote,
  IconRegenerate,
  IconThumbDown,
  IconThumbUp,
} from "./chat-icons";
import { userVisibleDegradeReasons } from "../../lib/workspace/degrade-display";
import { CitationRenderer, collectWebSources } from "./citation-renderer";
import { ProgressStatusLine } from "./progress-status-line";

export { ToolResultCard, ToolResultsPanel } from "./tool-result-card";

/** Completed-turn progress: always default-collapsed with expand/collapse control. */
function MessageProgressCard({
  locale,
  snapshot,
}: {
  locale: "zh-CN" | "en";
  snapshot: UiProgressSnapshot;
}) {
  // Force collapsed on first mount even if older snapshots had collapsed:false.
  const [collapsed, setCollapsed] = useState(true);
  const endedAtMs = snapshot.endedAtMs ?? snapshot.startedAtMs ?? Date.now();
  return (
    <ProgressStatusLine
      activities={snapshot.activities}
      collapsed={collapsed}
      locale={locale}
      mode={snapshot.mode}
      onToggleCollapsed={() => setCollapsed((c) => !c)}
      startedAtMs={snapshot.startedAtMs}
      endedAtMs={endedAtMs}
    />
  );
}

type MessageActionId = "copy" | "edit" | "note" | "regenerate";

function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
}

function getCopyableMessageContent(message: UiChatMessage) {
  if (message.role === "assistant") {
    const answerText = getAnswerBlockText(message.answerBlocks).trim();
    if (answerText) {
      return answerText;
    }
  }
  return message.content;
}

function getCapabilityLabel(locale: "zh-CN" | "en", cap: WorkspaceCapability) {
  return cap === "rag"
    ? formatUiMessage(locale, "workspaceChatCapRag")
    : formatUiMessage(locale, "workspaceChatCapSearch");
}

function messageHasSearch(message: UiChatMessage) {
  if (message.capabilities?.includes("search")) {
    return true;
  }
  return message.mode === "search" || message.mode === "rag+search";
}

function messageCapabilityChips(message: UiChatMessage): WorkspaceCapability[] {
  if (message.capabilities && message.capabilities.length > 0) {
    return message.capabilities.slice(0, 2);
  }
  if (message.mode === "rag") {
    return ["rag"];
  }
  if (message.mode === "search") {
    return ["search"];
  }
  if (message.mode === "rag+search") {
    return ["rag", "search"];
  }
  return [];
}

function getMessageActionIds(role: UiChatMessage["role"]): MessageActionId[] {
  if (role === "user") {
    return ["copy", "edit"];
  }
  return ["copy", "note", "regenerate"];
}

function getActionLabel(locale: "zh-CN" | "en", action: MessageActionId) {
  switch (action) {
    case "copy":
      return formatUiMessage(locale, "workspaceChatActionCopy");
    case "edit":
      return formatUiMessage(locale, "workspaceChatActionEdit");
    case "note":
      return formatUiMessage(locale, "workspaceChatActionAddToNote");
    case "regenerate":
      return formatUiMessage(locale, "workspaceChatActionRegenerate");
  }
}

function getActionIcon(action: MessageActionId): ReactNode {
  switch (action) {
    case "copy":
      return <IconCopy className={styles.messageActionIcon} />;
    case "edit":
      return <IconEdit className={styles.messageActionIcon} />;
    case "note":
      return <IconNote className={styles.messageActionIcon} />;
    case "regenerate":
      return <IconRegenerate className={styles.messageActionIcon} />;
  }
}




type ChatMessageListProps = {
  messages: UiChatMessage[];
  progress: {
    activities: ProgressEntry[];
    mode: WorkspaceChatMode | null;
    collapsed: boolean;
    startedAtMs: number | null;
    endedAtMs: number | null;
  };
  isStreaming: boolean;
  locale: "zh-CN" | "en";
  onSelectCitation: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources: (request: WorkspaceWebSourcesRequest) => void;
  onCopyMessage: (content: string) => void;
  onEditMessage: (content: string) => void;
  onSubmitFeedback: (messageId: string, rating: "up" | "down") => void;
  onToggleProgressCollapsed?: () => void;
};

export function ChatMessageList({
  messages,
  progress,
  isStreaming,
  locale,
  onSelectCitation,
  onOpenWebSources,
  onCopyMessage,
  onEditMessage,
  onSubmitFeedback,
  onToggleProgressCollapsed,
}: ChatMessageListProps) {
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const isNearBottomRef = useRef(true);
  const lastMessageIdRef = useRef<string | null>(null);
  const [showScrollToBottom, setShowScrollToBottom] = useState(false);
  const [feedbackRatings, setFeedbackRatings] = useState<Record<string, "up" | "down">>({});
  const [copiedMessageIds, setCopiedMessageIds] = useState<Record<string, boolean>>({});
  const copyResetTimeoutsRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    const timeouts = copyResetTimeoutsRef.current;
    return () => {
      timeouts.forEach((timeoutId) => window.clearTimeout(timeoutId));
      timeouts.clear();
    };
  }, []);

  // Auto-scroll to bottom on new messages / streaming / progress steps,
  // unless the user has scrolled away from the bottom.
  // Sending a new message (fresh user message appended) always snaps back
  // to the bottom and clears the "back to bottom" button.
  useEffect(() => {
    const transcript = transcriptRef.current;
    if (!transcript) {
      return;
    }
    const lastMessage = messages[messages.length - 1];
    const isNewUserMessage =
      lastMessage?.role === "user" && lastMessage.id !== lastMessageIdRef.current;
    if (lastMessage) {
      lastMessageIdRef.current = lastMessage.id;
    }
    if (isNewUserMessage) {
      isNearBottomRef.current = true;
      setShowScrollToBottom(false);
    }
    if (!isNearBottomRef.current) {
      return;
    }
    transcript.scrollTop = transcript.scrollHeight;
  }, [messages, isStreaming, progress.activities.length]);

  function handleTranscriptScroll() {
    const transcript = transcriptRef.current;
    if (!transcript) {
      return;
    }
    const nearBottom =
      transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 80;
    isNearBottomRef.current = nearBottom;
    setShowScrollToBottom(!nearBottom);
  }

  function handleScrollToBottomClick() {
    const transcript = transcriptRef.current;
    if (!transcript) {
      return;
    }
    isNearBottomRef.current = true;
    setShowScrollToBottom(false);
    const prefersReducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    transcript.scrollTo({
      top: transcript.scrollHeight,
      behavior: prefersReducedMotion ? "auto" : "smooth",
    });
  }

  function handleCitationSelect(message: UiChatMessage, citation: Citation) {
    if (message.sessionId && message.messageId !== null) {
      onSelectCitation({
        session_id: message.sessionId,
        message_id: message.messageId,
        citation,
      });
    }
  }

  function handleFeedback(messageId: string, rating: "up" | "down") {
    setFeedbackRatings((prev) => ({ ...prev, [messageId]: rating }));
    onSubmitFeedback(messageId, rating);
  }

  function handleCopyFeedback(messageId: string) {
    setCopiedMessageIds((prev) => ({ ...prev, [messageId]: true }));
    const existingTimeout = copyResetTimeoutsRef.current.get(messageId);
    if (existingTimeout !== undefined) {
      window.clearTimeout(existingTimeout);
    }
    const timeoutId = window.setTimeout(() => {
      copyResetTimeoutsRef.current.delete(messageId);
      setCopiedMessageIds((prev) => {
        const next = { ...prev };
        delete next[messageId];
        return next;
      });
    }, 1500);
    copyResetTimeoutsRef.current.set(messageId, timeoutId);
  }

  // Live process strip for the in-flight turn — after the latest user message.
  const liveProgressBeforeIndex = (() => {
    if (!progress.mode) {
      return -1;
    }
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i]?.role === "user") {
        return i + 1;
      }
    }
    return messages.length;
  })();

  const assistantAtLiveSlot =
    liveProgressBeforeIndex >= 0 && liveProgressBeforeIndex < messages.length
      ? messages[liveProgressBeforeIndex]
      : null;
  // Prefer message-bound snapshot once attached (avoids double cards after done).
  const showLiveProgress =
    progress.mode != null &&
    !(assistantAtLiveSlot?.role === "assistant" && assistantAtLiveSlot.progress);

  const liveProgressTimeline =
    showLiveProgress && progress.mode != null ? (
    <ProgressStatusLine
      activities={progress.activities}
      collapsed={progress.collapsed}
      locale={locale}
      mode={progress.mode}
      onToggleCollapsed={onToggleProgressCollapsed}
      startedAtMs={progress.startedAtMs}
      endedAtMs={progress.endedAtMs}
    />
  ) : null;

  return (
    <div
      className={styles.transcript}
      aria-label={formatUiMessage(locale, "workspaceTranscriptLabel")}
      onScroll={handleTranscriptScroll}
      ref={transcriptRef}
    >
      <div className={styles.transcriptInner}>
        {messages.map((message, index) => (
          <Fragment key={message.id}>
            {liveProgressBeforeIndex === index ? liveProgressTimeline : null}
            {message.role === "assistant" && message.progress ? (
              <MessageProgressCard locale={locale} snapshot={message.progress} />
            ) : null}
            <article
              className={[
                styles.message,
                message.role === "assistant" ? styles.messageAssistant : styles.messageUser,
              ]
                .filter(Boolean)
                .join(" ")}
              data-testid="chat-message"
              data-pending={message.pending}
              data-role={message.role}
            >
              <div
                className={[
                  styles.messageContent,
                  message.role === "assistant"
                    ? styles.messageContentAssistant
                    : styles.messageContentUser,
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                {(() => {
                  // No empty answer chrome before first character (progress card covers waiting).
                  const showAssistantBubble =
                    message.role !== "assistant" ||
                    !message.pending ||
                    message.content.trim().length > 0 ||
                    (message.answerBlocks?.length ?? 0) > 0;

                  if (message.role === "assistant" && !showAssistantBubble) {
                    return null;
                  }

                  return (
                    <>
                {message.role === "assistant"
                  ? (() => {
                      const chips = messageCapabilityChips(message);
                      if (chips.length === 0) {
                        return null;
                      }
                      return (
                        <div
                          className={styles.capabilityChipRow}
                          data-testid="mode-indicator"
                          data-mode={message.mode ?? "chat"}
                        >
                          {chips.map((cap) => (
                            <span
                              key={cap}
                              className={[
                                styles.modeBubbleTag,
                                cap === "rag"
                                  ? styles.modeBubbleTagRag
                                  : styles.modeBubbleTagSearch,
                              ].join(" ")}
                              data-testid={`capability-chip-${cap}`}
                              data-capability={cap}
                            >
                              {getCapabilityLabel(locale, cap)}
                            </span>
                          ))}
                        </div>
                      );
                    })()
                  : null}

                <div
                  className={[
                    styles.bubble,
                    message.role === "assistant"
                      ? [
                          styles.bubbleAssistant,
                          message.mode === "rag" || message.mode === "rag+search"
                            ? styles.bubbleAssistantRag
                            : message.mode === "search"
                              ? styles.bubbleAssistantSearch
                              : styles.bubbleAssistantGeneral,
                        ].join(" ")
                      : styles.bubbleUser,
                    message.pending ? styles.bubblePending : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  data-testid="workspace-answer-bubble"
                  data-mode={message.mode ?? "chat"}
                >
                  {message.role === "assistant" ? (
                    <>
                      <CitationRenderer
                        locale={locale}
                        message={message}
                        onOpenWebSources={onOpenWebSources}
                        onSelectCitation={(citation) => {
                          handleCitationSelect(message, citation);
                        }}
                      />
                      {message.pending ? (
                        <span
                          aria-hidden="true"
                          className={styles.streamCaret}
                          data-testid="stream-caret"
                        />
                      ) : null}
                    </>
                  ) : (
                    message.content || (message.pending ? "..." : "")
                  )}

                  {message.role === "assistant" && messageHasSearch(message) && !message.pending
                    ? (() => {
                        const webSources = collectWebSources(message.citations);
                        if (webSources.length === 0) {
                          return null;
                        }
                        return (
                          <button
                            className={styles.webSourceButton}
                            data-testid="citation-button"
                            onClick={() => onOpenWebSources({ sources: webSources })}
                            type="button"
                          >
                            {webSources.length === 1
                              ? formatUiMessage(locale, "workspaceSourcesCountOne")
                              : formatUiMessage(locale, "workspaceSourcesCountMany", {
                                  count: String(webSources.length),
                                })}
                          </button>
                        );
                      })()
                    : null}
                </div>
                    </>
                  );
                })()}

                {!(
                  message.role === "assistant" &&
                  message.pending &&
                  !message.content.trim() &&
                  (message.answerBlocks?.length ?? 0) === 0
                ) ? (
                <div className={styles.messageActions}>
                  {getMessageActionIds(message.role).map((action) => {
                    const copied = action === "copy" && Boolean(copiedMessageIds[message.id]);
                    const label = copied
                      ? formatUiMessage(locale, "workspaceChatActionCopied")
                      : getActionLabel(locale, action);
                    return (
                      <button
                        aria-label={label}
                        className={[
                          styles.messageActionButton,
                          copied ? styles.messageActionButtonActive : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        key={`${message.id}-${action}`}
                        title={label}
                        type="button"
                        onClick={() => {
                          if (action === "copy") {
                            onCopyMessage(getCopyableMessageContent(message));
                            handleCopyFeedback(message.id);
                          }
                          if (action === "edit" && message.role === "user") {
                            onEditMessage(message.content);
                          }
                        }}
                      >
                        {copied ? <IconCheck className={styles.messageActionIcon} /> : getActionIcon(action)}
                      </button>
                    );
                  })}
                  {message.role === "assistant" && !message.pending ? (
                    <>
                      <button
                        aria-label={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                        className={[
                          styles.messageActionButton,
                          feedbackRatings[message.id] === "up"
                            ? styles.messageActionButtonActive
                            : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        disabled={feedbackRatings[message.id] === "up"}
                        type="button"
                        onClick={() => handleFeedback(message.id, "up")}
                        title={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                      >
                        <IconThumbUp className={styles.messageActionIcon} />
                      </button>
                      <button
                        aria-label={formatUiMessage(locale, "workspaceChatActionThumbDown")}
                        className={[
                          styles.messageActionButton,
                          feedbackRatings[message.id] === "down"
                            ? styles.messageActionButtonActive
                            : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        disabled={feedbackRatings[message.id] === "down"}
                        type="button"
                        onClick={() => handleFeedback(message.id, "down")}
                        title={formatUiMessage(locale, "workspaceChatActionThumbDown")}
                      >
                        <IconThumbDown className={styles.messageActionIcon} />
                      </button>
                    </>
                  ) : null}
                </div>
                ) : null}

                {message.role === "assistant" &&
                (() => {
                  const visibleDegrade = userVisibleDegradeReasons(
                    message.degradeTrace.map((entry) => entry.reason),
                  );
                  if (!message.guarded && visibleDegrade.length === 0) {
                    return null;
                  }
                  return (
                  <div className={styles.messageNotice}>
                    {message.guarded ? (
                      <div className={styles.messageNoticeTitle}>
                        {formatUiMessage(locale, "workspaceGuardIntervened")}
                      </div>
                    ) : null}
                    {visibleDegrade.length > 0 ? (
                      <div className={styles.messageNoticeBody}>
                        {formatUiMessage(locale, "workspaceDegradeReasons", {
                          reasons: visibleDegrade.join(" / "),
                        })}
                      </div>
                    ) : null}
                  </div>
                  );
                })()}
              </div>
            </article>
          </Fragment>
        ))}

        {liveProgressBeforeIndex === messages.length ? liveProgressTimeline : null}
      </div>

      {showScrollToBottom ? (
        <button
          className={styles.scrollToBottomButton}
          data-testid="scroll-to-bottom"
          onClick={handleScrollToBottomClick}
          type="button"
        >
          {formatUiMessage(locale, "workspaceChatBackToBottom")}
        </button>
      ) : null}
    </div>
  );
}

