"use client";

import { useEffect, useState } from "react";
import { formatUiMessage } from "../lib/i18n/messages";
import { listWorkspaceSessionMessages } from "../lib/workspace/client";
import { mapTranscriptMessage } from "./chat-session/helpers";
import { useChatStream } from "./chat-session/use-chat-stream";
import { useMessageHistory } from "./chat-session/use-message-history";
import { useProgressTracker } from "./chat-session/use-progress-tracker";
import type {
  ProgressEntry,
  UiChatMessage,
  UiProgressSnapshot,
  UseChatSessionOptions,
  UseChatSessionResult,
} from "./chat-session/types";

export type {
  ProgressEntry,
  UiChatMessage,
  UiProgressSnapshot,
  UseChatSessionOptions,
  UseChatSessionResult,
};

export function useChatSession(options: UseChatSessionOptions): UseChatSessionResult {
  const {
    token,
    locale,
    workspaceId,
    sessionId,
    shareToken,
    initialMessages,
    onHistoryHydrated,
  } = options;

  const [error, setError] = useState("");
  const [activeSessionId, setActiveSessionId] = useState<string | null>(sessionId);
  const [isHydrating, setIsHydrating] = useState(
    Boolean(sessionId && token && !shareToken),
  );
  const [historyLoadFailed, setHistoryLoadFailed] = useState(false);

  const messageHistory = useMessageHistory(token, locale);
  const progressTracker = useProgressTracker(locale);
  const chatStream = useChatStream(
    options,
    messageHistory,
    progressTracker,
    setError,
    activeSessionId,
    setActiveSessionId,
  );

  useEffect(() => {
    setActiveSessionId(sessionId);
  }, [sessionId]);

  useEffect(() => {
    let cancelled = false;

    // Backend assigns session_id on stream start for new threads. Parent lifts that
    // into sessionId — do not tear down the in-flight progress card / local transcript
    // only when the prop exactly matches the stream that assigned it.
    const streamingSid = chatStream.streamingSessionIdRef.current;
    if (
      chatStream.isStreamingRef.current &&
      sessionId != null &&
      streamingSid === sessionId
    ) {
      setActiveSessionId(sessionId);
      chatStream.streamingSessionIdRef.current = sessionId;
      return () => {
        cancelled = true;
      };
    }

    if (chatStream.isStreamingRef.current) {
      chatStream.stop();
    }

    chatStream.resetStreamingTypewriter();
    setActiveSessionId(sessionId);
    messageHistory.reset();
    setError("");
    setHistoryLoadFailed(false);
    setIsHydrating(false);
    progressTracker.hide();
    chatStream.streamingSessionIdRef.current = sessionId;
    chatStream.streamingMessageIdRef.current = null;

    // Share local sessions: hydrate from parent-provided transcript (no PG list).
    if (shareToken) {
      if (initialMessages && initialMessages.length > 0) {
        messageHistory.setMessages(initialMessages);
      }
      return () => {
        cancelled = true;
      };
    }

    if (!sessionId || !token) {
      return () => {
        cancelled = true;
      };
    }

    const transcriptSessionId = sessionId;
    setIsHydrating(true);

    void (async () => {
      try {
        const response = await listWorkspaceSessionMessages(token, transcriptSessionId);

        if (cancelled) {
          return;
        }

        const hydratedMessages = response.messages.map((message) =>
          mapTranscriptMessage(message, locale),
        );
        messageHistory.setMessages(hydratedMessages);
        onHistoryHydrated?.(hydratedMessages);
      } catch {
        if (!cancelled) {
          setHistoryLoadFailed(true);
          setError(
            formatUiMessage(
              locale,
              workspaceId ? "workspaceChatLoadError" : "chat.loadError",
            ),
          );
        }
      } finally {
        if (!cancelled) {
          setIsHydrating(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
    // chatStream/messageHistory/progressTracker expose stable refs and useCallback
    // handlers; including the objects themselves would re-run this effect on every
    // render (new object identities) and trigger an infinite reset/render loop.
    // initialMessages is applied when sessionId/shareToken changes (parent remounts
    // or keys the pane). Do not list initialMessages as a dep — it updates after every
    // local transcript write and would wipe streaming state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, locale, workspaceId, sessionId, shareToken, onHistoryHydrated]);

  const toggleProgressCollapsed = progressTracker.toggleCollapsed;

  return {
    messages: messageHistory.messages,
    isStreaming: chatStream.isStreaming,
    isHydrating:
      isHydrating ||
      Boolean(sessionId && !shareToken && activeSessionId !== sessionId),
    historyLoadFailed,
    progress: progressTracker.progress,
    error: error || null,
    send: chatStream.send,
    stop: chatStream.stop,
    toggleProgressCollapsed,
  };
}
