"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  streamWorkspaceChat,
  type ChatRequest,
  type WorkspaceChatStreamEvent,
} from "../../lib/workspace/stream";
import { dispatchStreamEvent, type StreamEventHandlerDeps } from "./stream-event-handlers";
import {
  createStreamAssistantUpdates,
  type StreamAssistantUpdates,
} from "./stream-assistant-updates";
import { createStreamTypewriter, type StreamTypewriter } from "./stream-typewriter";
import type { MessageHistory } from "./use-message-history";
import type { ProgressTracker } from "./use-progress-tracker";
import type { UseChatSessionOptions } from "./types";

export function useChatStream(
  options: UseChatSessionOptions,
  messageHistory: MessageHistory,
  progressTracker: ProgressTracker,
  setError: React.Dispatch<React.SetStateAction<string>>,
  activeSessionId: string | null,
  setActiveSessionId: React.Dispatch<React.SetStateAction<string | null>>,
) {
  const tokenRef = useRef(options.token);
  const workspaceIdRef = useRef(options.workspaceId);
  const sessionIdRef = useRef(options.sessionId);
  const selectedSourceIdsRef = useRef(options.selectedSourceIds);
  const effectiveChatModeRef = useRef(options.effectiveChatMode);
  const localeRef = useRef(options.locale);
  const onSessionChangeRef = useRef(options.onSessionChange);
  const onSessionActivityRef = useRef(options.onSessionActivity);
  const activeSessionIdRef = useRef(activeSessionId);

  useEffect(() => {
    tokenRef.current = options.token;
    workspaceIdRef.current = options.workspaceId;
    sessionIdRef.current = options.sessionId;
    selectedSourceIdsRef.current = options.selectedSourceIds;
    effectiveChatModeRef.current = options.effectiveChatMode;
    localeRef.current = options.locale;
    onSessionChangeRef.current = options.onSessionChange;
    onSessionActivityRef.current = options.onSessionActivity;
    activeSessionIdRef.current = activeSessionId;
  });

  const [isStreaming, setIsStreaming] = useState(false);
  const [streamingMessageId, setStreamingMessageId] = useState<string | null>(null);

  const streamingSessionIdRef = useRef<string | null>(null);
  const streamingMessageIdRef = useRef<string | null>(null);
  const stopControllerRef = useRef<AbortController | null>(null);

  const streamEnginesRef = useRef<{
    assistant: StreamAssistantUpdates;
    typewriter: StreamTypewriter;
  } | null>(null);

  if (!streamEnginesRef.current) {
    const typewriterResetRef = { current: () => {} };

    const assistant = createStreamAssistantUpdates({
      messageHistory,
      streamingMessageIdRef,
      streamingSessionIdRef,
      effectiveChatModeRef,
      setActiveSessionId,
      onSessionChangeRef,
      setIsStreaming,
      setStreamingMessageId,
      resetStreamingTypewriter: () => typewriterResetRef.current(),
      streamingMessageId: null,
    });

    const typewriter = createStreamTypewriter({
      appendStreamingDisplayText: assistant.appendStreamingDisplayText,
      finalizeStreamingDone: assistant.finalizeStreamingDone,
    });

    typewriterResetRef.current = typewriter.resetStreamingTypewriter;
    streamEnginesRef.current = { assistant, typewriter };
  }

  const { assistant: assistantUpdates, typewriter } = streamEnginesRef.current;

  const handleStreamEvent = useCallback(
    (event: WorkspaceChatStreamEvent) => {
      const deps: StreamEventHandlerDeps = {
        progressTracker,
        setError,
        setActiveSessionId,
        setIsStreaming,
        setStreamingMessageId,
        streamingSessionIdRef,
        streamingMessageIdRef,
        onSessionChangeRef,
        streamingMessageId,
        beginAnswerStreaming: assistantUpdates.beginAnswerStreaming,
        ensureStreamingAssistant: assistantUpdates.ensureStreamingAssistant,
        markTokenReceived: typewriter.markTokenReceived,
        enqueueStreamingText: typewriter.enqueueStreamingText,
        handleDoneWithTypewriter: typewriter.handleDoneWithTypewriter,
        resetStreamingTypewriter: typewriter.resetStreamingTypewriter,
        clearPendingStreamingAssistant: assistantUpdates.clearPendingStreamingAssistant,
      };

      dispatchStreamEvent(deps, event);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [progressTracker, setError, setActiveSessionId, streamingMessageId],
  );

  const send = useCallback(
    (query: string) => {
      const trimmedQuery = query.trim();

      if (!trimmedQuery || isStreaming || !tokenRef.current) {
        return;
      }

      const nextAssistantId = `assistant-${Date.now()}`;
      const requestSessionId = activeSessionIdRef.current ?? sessionIdRef.current;

      setError("");
      setIsStreaming(true);
      setStreamingMessageId(nextAssistantId);
      streamingMessageIdRef.current = nextAssistantId;
      typewriter.resetStreamingTypewriter();
      onSessionActivityRef.current?.();

      messageHistory.setMessages((current) => [
        ...current,
        {
          id: `user-${Date.now()}`,
          role: "user",
          mode: null,
          content: trimmedQuery,
          answerBlocks: [],
          citations: [],
          degradeTrace: [],
          guarded: false,
          messageId: null,
          sessionId: requestSessionId,
          toolResults: [],
        },
      ]);
      progressTracker.show(effectiveChatModeRef.current);

      const controller = new AbortController();
      stopControllerRef.current = controller;

      void (async () => {
        try {
          await streamWorkspaceChat(
            tokenRef.current,
            {
              query: trimmedQuery,
              notebook_id: workspaceIdRef.current,
              session_id: requestSessionId,
              agent_type: effectiveChatModeRef.current,
              doc_scope: selectedSourceIdsRef.current,
              messages: [],
              stream: true,
            } as ChatRequest,
            handleStreamEvent,
            { signal: controller.signal },
          );
        } catch (submitError) {
          if (submitError instanceof Error && submitError.name === "AbortError") {
            return;
          }
          progressTracker.hide();
          typewriter.resetStreamingTypewriter();
          assistantUpdates.clearPendingStreamingAssistant();
          setError(
            submitError instanceof Error
              ? submitError.message
              : formatUiMessage(localeRef.current, "workspaceStreamError"),
          );
          setIsStreaming(false);
          setStreamingMessageId(null);
          streamingMessageIdRef.current = null;
        } finally {
          if (stopControllerRef.current === controller) {
            stopControllerRef.current = null;
          }
        }
      })();
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [isStreaming, messageHistory.setMessages, progressTracker, setError, handleStreamEvent],
  );

  const stop = useCallback(() => {
    const controller = stopControllerRef.current;
    if (!controller) {
      return;
    }

    stopControllerRef.current = null;
    controller.abort();

    progressTracker.hide();
    typewriter.resetStreamingTypewriter();
    assistantUpdates.clearPendingStreamingAssistant();
    setIsStreaming(false);
    setStreamingMessageId(null);
    streamingMessageIdRef.current = null;
  }, [progressTracker]);

  useEffect(() => {
    const engines = streamEnginesRef.current;

    return () => {
      engines?.typewriter.resetStreamingTypewriter();
      engines?.typewriter.stopStreamingTypewriter();
      if (stopControllerRef.current) {
        stopControllerRef.current.abort();
        stopControllerRef.current = null;
      }
    };
  }, []);

  return {
    isStreaming,
    send,
    stop,
    resetStreamingTypewriter: typewriter.resetStreamingTypewriter,
    streamingSessionIdRef,
    streamingMessageIdRef,
  };
}
