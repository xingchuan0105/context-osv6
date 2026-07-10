"use client";

import { Fragment, type ReactNode, useEffect, useRef, useState } from "react";
import { formatUiMessage } from "../../lib/i18n/messages";
import type {
  WorkspaceCitationRequest,
  WorkspaceWebSourcesRequest,
} from "../../lib/workspace/model";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import {
  type AnswerBlock,
  type Citation,
} from "../../lib/workspace/stream";
import styles from "./workspace-chat.module.css";
import type { ProgressEntry, UiChatMessage } from "../../hooks/use-chat-session";
import {
  IconChatEmpty,
  IconCopy,
  IconEdit,
  IconNote,
  IconRegenerate,
  IconThumbDown,
  IconThumbUp,
} from "./chat-icons";
import { CitationRenderer, collectWebSources, getCitationAnchorRect } from "./citation-renderer";
import { ProgressTimeline } from "./progress-timeline";

export { ToolResultCard, ToolResultsPanel } from "./tool-result-card";

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
  /** Current mode label for empty-state hint (U12). */
  activeModeLabel?: string;
  onToggleProgressCollapsed: () => void;
  onSelectCitation: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources: (request: WorkspaceWebSourcesRequest) => void;
  onCopyMessage: (content: string) => void;
  onEditMessage: (content: string) => void;
  onSubmitFeedback: (messageId: string, rating: "up" | "down") => void;
};

export function ChatMessageList({
  messages,
  progress,
  isStreaming,
  locale,
  activeModeLabel,
  onToggleProgressCollapsed,
  onSelectCitation,
  onOpenWebSources,
  onCopyMessage,
  onEditMessage,
  onSubmitFeedback,
}: ChatMessageListProps) {
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const [feedbackRatings, setFeedbackRatings] = useState<Record<string, "up" | "down">>({});

  // Auto-scroll to bottom on new messages / streaming / progress steps
  useEffect(() => {
    const transcript = transcriptRef.current;
    if (!transcript) {
      return;
    }
    transcript.scrollTop = transcript.scrollHeight;
  }, [messages, isStreaming, progress.activities.length]);

  function handleCitationSelect(message: UiChatMessage, citation: Citation, target?: HTMLElement | null) {
    if (message.sessionId && message.messageId !== null) {
      onSelectCitation({
        session_id: message.sessionId,
        message_id: message.messageId,
        citation,
        anchorRect: target ? getCitationAnchorRect(target) : null,
      });
    }
  }

  function handleFeedback(messageId: string, rating: "up" | "down") {
    setFeedbackRatings((prev) => ({ ...prev, [messageId]: rating }));
    onSubmitFeedback(messageId, rating);
  }

  // Grok layout: process strip sits above the current-turn assistant answer when present.
  const progressBeforeIndex = (() => {
    if (!progress.mode) {
      return -1;
    }
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i]?.role === "assistant") {
        return i;
      }
    }
    return messages.length;
  })();

  const progressTimeline =
    progress.mode != null ? (
      <ProgressTimeline
        activities={progress.activities}
        collapsed={progress.collapsed}
        locale={locale}
        mode={progress.mode}
        startedAtMs={progress.startedAtMs}
        endedAtMs={progress.endedAtMs}
        onToggleCollapsed={onToggleProgressCollapsed}
      />
    ) : null;

  return (
    <div
      className={styles.transcript}
      aria-label={formatUiMessage(locale, "workspaceTranscriptLabel")}
      ref={transcriptRef}
    >
      <div className={styles.transcriptInner}>
        {messages.length === 0 && !progress.mode ? (
          <div className={styles.emptyStateCard} data-testid="workspace-chat-empty">
            <div className={styles.emptyStateIcon} aria-hidden="true">
              <IconChatEmpty />
            </div>
            <p className={styles.emptyStateTitle}>
              {formatUiMessage(locale, "workspaceNoMessages")}
            </p>
            <p className={styles.emptyState}>
              {formatUiMessage(locale, "workspaceEmptyStateBody")}
            </p>
            {activeModeLabel ? (
              <p className={styles.emptyStateModeHint}>
                {formatUiMessage(locale, "workspaceEmptyStateModeHint", {
                  mode: activeModeLabel,
                })}
              </p>
            ) : null}
          </div>
        ) : null}

        {messages.map((message, index) => (
          <Fragment key={message.id}>
            {progressBeforeIndex === index ? progressTimeline : null}
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
                {message.role === "assistant" ? (
                  <div
                    className={[
                      styles.modeBubbleTag,
                      message.mode === "rag"
                        ? styles.modeBubbleTagRag
                        : message.mode === "search"
                          ? styles.modeBubbleTagSearch
                          : message.mode === "write"
                            ? styles.modeBubbleTagWrite
                            : styles.modeBubbleTagGeneral,
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    data-testid="mode-indicator"
                    data-mode={message.mode ?? "chat"}
                  >
                    <span>{getModeLabel(locale, message.mode ?? "chat")}</span>
                    <span className={styles.modeBubbleTagCode}>
                      {getModeCode(message.mode ?? "chat")}
                    </span>
                  </div>
                ) : null}

                <div
                  className={[
                    styles.bubble,
                    message.role === "assistant"
                      ? [
                          styles.bubbleAssistant,
                          message.mode === "rag"
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
                        onSelectCitation={(citation, target) => {
                          handleCitationSelect(message, citation, target);
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

                  {message.role === "assistant" && message.mode === "search" && !message.pending
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
                            {locale === "zh-CN"
                              ? `${webSources.length} 个来源`
                              : `${webSources.length} source${webSources.length > 1 ? "s" : ""}`}
                          </button>
                        );
                      })()
                    : null}
                </div>

                <div className={styles.messageActions}>
                  {getMessageActionIds(message.role).map((action) => {
                    const label = getActionLabel(locale, action);
                    return (
                      <button
                        aria-label={label}
                        className={styles.messageActionButton}
                        key={`${message.id}-${action}`}
                        title={label}
                        type="button"
                        onClick={() => {
                          if (action === "copy") {
                            onCopyMessage(getCopyableMessageContent(message));
                          }
                          if (action === "edit" && message.role === "user") {
                            onEditMessage(message.content);
                          }
                        }}
                      >
                        {getActionIcon(action)}
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

                {message.role === "assistant" &&
                (message.guarded || message.degradeTrace.length > 0) ? (
                  <div className={styles.messageNotice}>
                    {message.guarded ? (
                      <div className={styles.messageNoticeTitle}>
                        {formatUiMessage(locale, "workspaceGuardIntervened")}
                      </div>
                    ) : null}
                    {message.degradeTrace.length > 0 ? (
                      <div className={styles.messageNoticeBody}>
                        {formatUiMessage(locale, "workspaceDegradeReasons", {
                          reasons: message.degradeTrace.map((entry) => entry.reason).join(" / "),
                        })}
                      </div>
                    ) : null}
                  </div>
                ) : null}
              </div>
            </article>
          </Fragment>
        ))}

        {progressBeforeIndex === messages.length ? progressTimeline : null}
      </div>
    </div>
  );
}

