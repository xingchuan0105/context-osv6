"use client";

import { useEffect, useRef, useState } from "react";
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
import type { ChatMessage, ProgressEntry } from "../../hooks/use-chat-session";
import { CitationRenderer, collectWebSources, getCitationAnchorRect } from "./citation-renderer";
import { ProgressTimeline } from "./progress-timeline";

export { ToolResultCard, ToolResultsPanel } from "./tool-result-card";

function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
}

function getCopyableMessageContent(message: ChatMessage) {
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
    case "chat":
    default:
      return "chat";
  }
}

function getMessageActions(locale: "zh-CN" | "en", role: ChatMessage["role"]) {
  if (role === "user") {
    return [
      formatUiMessage(locale, "workspaceChatActionCopy"),
      formatUiMessage(locale, "workspaceChatActionEdit"),
    ];
  }
  return [
    formatUiMessage(locale, "workspaceChatActionCopy"),
    formatUiMessage(locale, "workspaceChatActionAddToNote"),
    formatUiMessage(locale, "workspaceChatActionRegenerate"),
  ];
}




type ChatMessageListProps = {
  messages: ChatMessage[];
  progress: { activities: ProgressEntry[]; mode: WorkspaceChatMode | null; collapsed: boolean };
  isStreaming: boolean;
  locale: "zh-CN" | "en";
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
  onToggleProgressCollapsed,
  onSelectCitation,
  onOpenWebSources,
  onCopyMessage,
  onEditMessage,
  onSubmitFeedback,
}: ChatMessageListProps) {
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const [feedbackRatings, setFeedbackRatings] = useState<Record<string, "up" | "down">>({});

  // Auto-scroll to bottom on new messages / streaming
  useEffect(() => {
    const transcript = transcriptRef.current;
    if (!transcript) {
      return;
    }
    transcript.scrollTop = transcript.scrollHeight;
  }, [messages, isStreaming]);

  function handleCitationSelect(message: ChatMessage, citation: Citation, target?: HTMLElement | null) {
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

  return (
    <div
      className={styles.transcript}
      aria-label={formatUiMessage(locale, "workspaceTranscriptLabel")}
      ref={transcriptRef}
    >
      <div className={styles.transcriptInner}>
        {messages.map((message) => (
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
            key={message.id}
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
                  <CitationRenderer
                    locale={locale}
                    message={message}
                    onOpenWebSources={onOpenWebSources}
                    onSelectCitation={(citation, target) => {
                      handleCitationSelect(message, citation, target);
                    }}
                  />
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
                {getMessageActions(locale, message.role).map((action) => (
                  <button
                    className={styles.messageActionButton}
                    key={`${message.id}-${action}`}
                    type="button"
                    onClick={() => {
                      if (action === formatUiMessage(locale, "workspaceChatActionCopy")) {
                        onCopyMessage(getCopyableMessageContent(message));
                      }
                      if (
                        message.role === "user" &&
                        action === formatUiMessage(locale, "workspaceChatActionEdit")
                      ) {
                        onEditMessage(message.content);
                      }
                    }}
                  >
                    {action}
                  </button>
                ))}
                {message.role === "assistant" && !message.pending ? (
                  <>
                    <button
                      aria-label={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                      className={[
                        styles.messageActionButton,
                        feedbackRatings[message.id] === "up" ? styles.messageActionButtonActive : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      disabled={feedbackRatings[message.id] === "up"}
                      type="button"
                      onClick={() => handleFeedback(message.id, "up")}
                      title={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                    >
                      {feedbackRatings[message.id] === "up" ? "👍" : "🤙"}
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
                      {feedbackRatings[message.id] === "down" ? "👎" : "🫣"}
                    </button>
                  </>
                ) : null}
              </div>

              {message.role === "assistant" && (message.guarded || message.degradeTrace.length > 0) ? (
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
        ))}

        {progress.mode ? (
          <ProgressTimeline
            activities={progress.activities}
            collapsed={progress.collapsed}
            locale={locale}
            mode={progress.mode}
            onToggleCollapsed={onToggleProgressCollapsed}
          />
        ) : null}
      </div>
    </div>
  );
}

