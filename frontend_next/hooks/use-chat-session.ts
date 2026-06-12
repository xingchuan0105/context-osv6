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
  UseChatSessionOptions,
  UseChatSessionResult,
} from "./chat-session/types";

export type { ProgressEntry, UiChatMessage, UseChatSessionOptions, UseChatSessionResult };

export function useChatSession(options: UseChatSessionOptions): UseChatSessionResult {
  const { token, locale, sessionId } = options;

  const [error, setError] = useState("");
  const [activeSessionId, setActiveSessionId] = useState<string | null>(sessionId);

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

    chatStream.resetStreamingTypewriter();
    setActiveSessionId(sessionId);
    messageHistory.reset();
    setError("");
    progressTracker.hide();
    chatStream.streamingSessionIdRef.current = sessionId;
    chatStream.streamingMessageIdRef.current = null;

    if (!sessionId || !token) {
      return () => {
        cancelled = true;
      };
    }

    const transcriptSessionId = sessionId;

    void (async () => {
      try {
        const response = await listWorkspaceSessionMessages(token, transcriptSessionId);

        if (cancelled) {
          return;
        }

        messageHistory.setMessages(response.messages.map(mapTranscriptMessage));
      } catch {
        if (!cancelled) {
          setError(formatUiMessage(locale, "workspaceChatLoadError"));
        }
      }
    })();

    return () => {
      cancelled = true;
    };
    // chatStream/messageHistory/progressTracker expose stable refs and useCallback
    // handlers; including the objects themselves would re-run this effect on every
    // render (new object identities) and trigger an infinite reset/render loop.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, locale, sessionId]);

  const toggleProgressCollapsed = progressTracker.toggleCollapsed;

  return {
    messages: messageHistory.messages,
    isStreaming: chatStream.isStreaming,
    progress: progressTracker.progress,
    error: error || null,
    send: chatStream.send,
    stop: chatStream.stop,
    toggleProgressCollapsed,
  };
}
