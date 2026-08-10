"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { describeProductError } from "../../lib/auth/errors";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  buildClientContext,
  deriveAgentTypeLabel,
  normalizeCapabilities,
  type WorkspaceCapability,
} from "../../lib/workspace/capabilities";
import { streamChat } from "../../lib/runtime/transport";
import type { ChatEvent } from "../../lib/contracts";
import type { ChatRequest } from "../../lib/workspace/stream";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
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
  const capabilitiesRef = useRef<WorkspaceCapability[]>(
    normalizeCapabilities(options.capabilities),
  );
  const effectiveChatModeRef = useRef<WorkspaceChatMode>(
    deriveAgentTypeLabel(capabilitiesRef.current),
  );
  const localeRef = useRef(options.locale);
  const onSessionChangeRef = useRef(options.onSessionChange);
  const onSessionActivityRef = useRef(options.onSessionActivity);
  const shareTokenRef = useRef(options.shareToken ?? null);
  const turnstileTokenRef = useRef(options.turnstileToken ?? null);
  const onTranscriptChangeRef = useRef(options.onTranscriptChange);
  const activeSessionIdRef = useRef(activeSessionId);

  useEffect(() => {
    tokenRef.current = options.token;
    workspaceIdRef.current = options.workspaceId;
    sessionIdRef.current = options.sessionId;
    selectedSourceIdsRef.current = options.selectedSourceIds;
    const nextCaps = normalizeCapabilities(options.capabilities);
    capabilitiesRef.current = nextCaps;
    effectiveChatModeRef.current = deriveAgentTypeLabel(nextCaps);
    localeRef.current = options.locale;
    onSessionChangeRef.current = options.onSessionChange;
    onSessionActivityRef.current = options.onSessionActivity;
    shareTokenRef.current = options.shareToken ?? null;
    turnstileTokenRef.current = options.turnstileToken ?? null;
    onTranscriptChangeRef.current = options.onTranscriptChange;
    activeSessionIdRef.current = activeSessionId;
  }, [
    options.token,
    options.workspaceId,
    options.sessionId,
    options.selectedSourceIds,
    options.capabilities,
    options.locale,
    options.onSessionChange,
    options.onSessionActivity,
    options.shareToken,
    options.turnstileToken,
    options.onTranscriptChange,
    activeSessionId,
  ]);

  const [isStreaming, setIsStreamingState] = useState(false);
  const [streamingMessageId, setStreamingMessageId] = useState<string | null>(null);

  const streamingSessionIdRef = useRef<string | null>(null);
  const streamingMessageIdRef = useRef<string | null>(null);
  /** True while a turn is in flight (including wait before first token). Used to avoid
   * sessionId-driven transcript reload wiping the live progress card. */
  const isStreamingRef = useRef(false);
  const setIsStreaming = useCallback((value: React.SetStateAction<boolean>) => {
    setIsStreamingState((prev) => {
      const next = typeof value === "function" ? value(prev) : value;
      isStreamingRef.current = next;
      return next;
    });
  }, []);
  const stopControllerRef = useRef<AbortController | null>(null);

  const streamEnginesRef = useRef<{
    assistant: StreamAssistantUpdates;
    typewriter: StreamTypewriter;
  } | null>(null);
  const progressTrackerRef = useRef(progressTracker);
  progressTrackerRef.current = progressTracker;

  if (!streamEnginesRef.current) {
    const typewriterResetRef = { current: () => {} };

    const assistant = createStreamAssistantUpdates({
      messageHistory,
      streamingMessageIdRef,
      streamingSessionIdRef,
      effectiveChatModeRef,
      capabilitiesRef,
      setActiveSessionId,
      onSessionChangeRef,
      setIsStreaming,
      setStreamingMessageId,
      resetStreamingTypewriter: () => typewriterResetRef.current(),
      streamingMessageId: null,
    });

    const typewriter = createStreamTypewriter({
      appendStreamingDisplayText: assistant.appendStreamingDisplayText,
      finalizeStreamingDone: (event, progressSnapshot) => {
        assistant.finalizeStreamingDone(event, progressSnapshot);
        // Live card → message-bound card; clear the singleton so we don't double-render.
        progressTrackerRef.current.hide();
      },
    });

    typewriterResetRef.current = typewriter.resetStreamingTypewriter;
    streamEnginesRef.current = { assistant, typewriter };
  }

  const { assistant: assistantUpdates, typewriter } = streamEnginesRef.current;

  const handleStreamEvent = useCallback(
    (event: ChatEvent) => {
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

      const shareToken = shareTokenRef.current?.trim() || "";
      const authToken = tokenRef.current?.trim() || "";
      if (!trimmedQuery || isStreaming || (!authToken && !shareToken)) {
        return;
      }

      const nextAssistantId = `assistant-${Date.now()}`;
      let requestSessionId = activeSessionIdRef.current ?? sessionIdRef.current;
      // Share chats are not PG-sessioned; allocate a stable client session id for multi-turn.
      if (shareToken && !requestSessionId) {
        requestSessionId = crypto.randomUUID();
        setActiveSessionId(requestSessionId);
        onSessionChangeRef.current?.(requestSessionId);
      }

      setError("");
      setIsStreaming(true);
      setStreamingMessageId(nextAssistantId);
      streamingMessageIdRef.current = nextAssistantId;
      typewriter.resetStreamingTypewriter();
      onSessionActivityRef.current?.();

      // U9: new thread (no session yet) must not keep previous session transcript.
      // Do NOT insert an empty assistant bubble here — product rule: no answer frame
      // until the first answer character (token / typewriter). Progress card alone
      // covers "work in progress" until ensureStreamingAssistant runs.
      const now = Date.now();
      const turnCapabilities = [...capabilitiesRef.current];
      const agentType = deriveAgentTypeLabel(turnCapabilities);
      effectiveChatModeRef.current = agentType;

      // Prior turns for share multi-turn (server skips PG persistence for source_type=share).
      const priorMessages = shareToken
        ? messageHistory.messages
            .filter((m) => !m.pending && m.content.trim().length > 0)
            .map((m) => ({ role: m.role, content: m.content }))
        : [];

      messageHistory.setMessages((current) => {
        const base = requestSessionId == null && !shareToken ? [] : current;
        return [
          ...base,
          {
            id: `user-${now}`,
            role: "user",
            mode: null,
            capabilities: [],
            content: trimmedQuery,
            answerBlocks: [],
            citations: [],
            degradeTrace: [],
            guarded: false,
            messageId: null,
            sessionId: requestSessionId,
            toolResults: [],
          },
        ];
      });
      // Reserve streaming id so the first token attaches to a stable assistant row.
      streamingMessageIdRef.current = nextAssistantId;
      progressTracker.show(agentType);

      const controller = new AbortController();
      stopControllerRef.current = controller;

      void (async () => {
        try {
          const turnstile = turnstileTokenRef.current?.trim() || "";
          const extraHeaders: Record<string, string> = {};
          if (turnstile) {
            extraHeaders["cf-turnstile-response"] = turnstile;
          }
          await streamChat(
            authToken,
            {
              query: trimmedQuery,
              workspace_id: workspaceIdRef.current,
              session_id: requestSessionId,
              agent_type: agentType,
              capabilities: turnCapabilities,
              client_context: buildClientContext(),
              doc_scope: selectedSourceIdsRef.current,
              messages: priorMessages,
              stream: true,
              ...(shareToken
                ? {
                    source_type: "share",
                    source_token: shareToken,
                    turnstile_token: turnstile || undefined,
                  }
                : {}),
            } as ChatRequest,
            handleStreamEvent,
            { signal: controller.signal, extraHeaders },
          );
        } catch (submitError) {
          if (submitError instanceof Error && submitError.name === "AbortError") {
            return;
          }
          progressTracker.hide();
          typewriter.resetStreamingTypewriter();
          assistantUpdates.clearPendingStreamingAssistant();
          setError(
            describeProductError(
              formatUiMessage(localeRef.current, "workspaceStreamError"),
              submitError,
              localeRef.current,
            ),
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
    [isStreaming, messageHistory.setMessages, progressTracker, setError, handleStreamEvent, setIsStreaming],
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
  }, [progressTracker, setIsStreaming]);

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
    isStreamingRef,
    send,
    stop,
    resetStreamingTypewriter: typewriter.resetStreamingTypewriter,
    streamingSessionIdRef,
    streamingMessageIdRef,
  };
}
