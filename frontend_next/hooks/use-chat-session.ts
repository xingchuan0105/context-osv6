"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { formatUiMessage } from "../lib/i18n/messages";
import {
  listWorkspaceSessionMessages,
  type WorkspaceChatMessage,
} from "../lib/workspace/client";
import type { WorkspaceChatMode } from "../lib/workspace/ui-store";
import {
  streamWorkspaceChat,
  type AnswerBlock,
  type Citation,
  type DegradeTraceItem,
  type ProgressSourcePreview,
  type ToolResult,
  type WorkspaceChatStreamEvent,
} from "../lib/workspace/stream";

// =============================================================================
// Types
// =============================================================================

export type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  mode: WorkspaceChatMode | null;
  content: string;
  answerBlocks: AnswerBlock[];
  citations: Citation[];
  degradeTrace: DegradeTraceItem[];
  guarded: boolean;
  messageId: number | null;
  pending?: boolean;
  sessionId: string | null;
  toolResults: ToolResult[];
};

export type ProgressEntry = {
  id: string;
  phase: string;
  title: string;
  detail: string | null;
  counts: Record<string, number>;
  sourcesPreview: ProgressSourcePreview[];
  timestamp: string | null;
};

export type UseChatSessionOptions = {
  token: string;
  workspaceId: string;
  sessionId: string | null;
  selectedSourceIds: string[];
  effectiveChatMode: WorkspaceChatMode;
  locale: "zh-CN" | "en";
  onSessionChange?: (sessionId: string | null) => void;
  onSessionActivity?: () => void;
};

export type UseChatSessionResult = {
  messages: ChatMessage[];
  isStreaming: boolean;
  progress: {
    activities: ProgressEntry[];
    mode: WorkspaceChatMode | null;
    collapsed: boolean;
  };
  error: string | null;
  send: (query: string) => void;
  stop: () => void;
  toggleProgressCollapsed: () => void;
};

type PendingDoneEvent = Extract<WorkspaceChatStreamEvent, { kind: "done" }>;

// =============================================================================
// Constants
// =============================================================================

const STREAM_TYPEWRITER_CHARS_PER_TICK = 8;
const STREAM_TYPEWRITER_INTERVAL_MS = 16;
const STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE = 80;

// =============================================================================
// Helpers
// =============================================================================

function normalizeMessageMode(mode: string | null | undefined): WorkspaceChatMode | null {
  if (mode === "general" || mode === "chat") {
    return "chat";
  }
  if (mode === "rag" || mode === "search") {
    return mode;
  }
  return null;
}

function mapTranscriptMessage(message: WorkspaceChatMessage): ChatMessage {
  return {
    id: String(message.id),
    role: message.role === "assistant" ? "assistant" : "user",
    mode: message.role === "assistant" ? normalizeMessageMode(message.agent_id) : null,
    content: message.content,
    answerBlocks: message.answer_blocks ?? [],
    citations: message.citations ?? [],
    degradeTrace: [],
    guarded: false,
    messageId: message.id,
    pending: false,
    sessionId: message.session_id,
    toolResults: message.tool_results ?? [],
  };
}

function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
}

function getAnswerText(content: string, blocks: AnswerBlock[]) {
  const blockText = getAnswerBlockText(blocks);
  return content.trim().length > 0 ? content : blockText;
}

function getStreamingDisplayText(content: string, blocks: AnswerBlock[]) {
  const blockText = getAnswerBlockText(blocks);
  return blockText || content;
}

function getPrefersReducedStreamingMotion() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function hasGuardrailIntervention(guardReport: unknown) {
  if (!guardReport || typeof guardReport !== "object") {
    return false;
  }
  const candidate = guardReport as {
    blocked?: unknown;
    output_results?: unknown;
  };
  if (candidate.blocked === true) {
    return true;
  }
  if (!Array.isArray(candidate.output_results)) {
    return false;
  }
  return candidate.output_results.some((result) => {
    if (!result || typeof result !== "object") {
      return false;
    }
    const outputResult = result as {
      passed?: unknown;
      action?: unknown;
    };
    if (outputResult.passed === false) {
      return true;
    }
    if (typeof outputResult.action !== "string") {
      return false;
    }
    return outputResult.action.trim().toLowerCase() !== "allow";
  });
}

function normalizeStreamMessageId(messageId: number) {
  return messageId > 0 ? messageId : null;
}

function getAssistantMessageKey(messageId: number) {
  return `assistant-${messageId}`;
}

function isResearchMode(mode: WorkspaceChatMode) {
  return mode === "rag" || mode === "search";
}

function getInitialProgressEntry(locale: "zh-CN" | "en", mode: WorkspaceChatMode): ProgressEntry {
  if (locale === "zh-CN") {
    if (mode === "rag") {
      return {
        id: "progress-initial",
        phase: "planning",
        title: "正在分析问题并准备检索知识库",
        detail: "系统正在规划检索范围与证据路径。",
        counts: {},
        sourcesPreview: [],
        timestamp: null,
      };
    }
    return {
      id: "progress-initial",
      phase: "planning",
      title: "正在生成网络搜索计划",
      detail: "系统正在拆解问题并准备搜索网页来源。",
      counts: {},
      sourcesPreview: [],
      timestamp: null,
    };
  }
  if (mode === "rag") {
    return {
      id: "progress-initial",
      phase: "planning",
      title: "Preparing knowledge retrieval",
      detail: "Building a retrieval plan and evidence path.",
      counts: {},
      sourcesPreview: [],
      timestamp: null,
    };
  }
  return {
    id: "progress-initial",
    phase: "planning",
    title: "Preparing a web research plan",
    detail: "Breaking down the request before searching the web.",
    counts: {},
    sourcesPreview: [],
    timestamp: null,
  };
}

// =============================================================================
// Internal Hook 1: Message History
// =============================================================================

function useMessageHistory(token: string, locale: "zh-CN" | "en") {
  const [messages, setMessages] = useState<ChatMessage[]>([]);

  const loadSession = useCallback(
    async (sessionId: string) => {
      if (!token || !sessionId) {
        setMessages([]);
        return;
      }
      const response = await listWorkspaceSessionMessages(token, sessionId);
      setMessages(response.messages.map(mapTranscriptMessage));
    },
    [token, locale],
  );

  const reset = useCallback(() => {
    setMessages([]);
  }, []);

  return { messages, setMessages, loadSession, reset };
}

// =============================================================================
// Internal Hook 2: Progress Tracker
// =============================================================================

function useProgressTracker(locale: "zh-CN" | "en") {
  const [mode, setMode] = useState<WorkspaceChatMode | null>(null);
  const [activities, setActivities] = useState<ProgressEntry[]>([]);
  const [collapsed, setCollapsed] = useState(true);
  const modeRef = useRef<WorkspaceChatMode | null>(null);

  const show = useCallback(
    (m: WorkspaceChatMode) => {
      modeRef.current = m;
      setMode(m);
      setActivities(isResearchMode(m) ? [getInitialProgressEntry(locale, m)] : []);
      setCollapsed(true);
    },
    [locale],
  );

  const hide = useCallback(() => {
    modeRef.current = null;
    setMode(null);
    setActivities([]);
    setCollapsed(true);
  }, []);

  const addActivity = useCallback(
    (event: Extract<WorkspaceChatStreamEvent, { kind: "activity" }>) => {
      setActivities((current) => [
        ...current,
        {
          id: `${event.phase}-${current.length}-${event.timestamp ?? Date.now()}`,
          phase: event.phase,
          title: event.title,
          detail: event.detail ?? null,
          counts: event.counts,
          sourcesPreview: event.sources_preview,
          timestamp: event.timestamp ?? null,
        },
      ]);
    },
    [],
  );

  const addReasoning = useCallback(
    (content: string) => {
      setActivities((current) => [
        ...current,
        {
          id: `reasoning-${current.length}-${Date.now()}`,
          phase: "reasoning",
          title: locale === "zh-CN" ? "正在整理思路" : "Reasoning summary",
          detail: content,
          counts: {},
          sourcesPreview: [],
          timestamp: null,
        },
      ]);
    },
    [locale],
  );

  const toggleCollapsed = useCallback(() => {
    setCollapsed((c) => !c);
  }, []);

  return {
    progress: { mode, activities, collapsed },
    show,
    hide,
    addActivity,
    addReasoning,
    toggleCollapsed,
    modeRef,
  };
}

// =============================================================================
// Internal Hook 3: Chat Stream
// =============================================================================

function useChatStream(
  options: UseChatSessionOptions,
  messageHistory: ReturnType<typeof useMessageHistory>,
  progressTracker: ReturnType<typeof useProgressTracker>,
  setError: React.Dispatch<React.SetStateAction<string>>,
  activeSessionId: string | null,
  setActiveSessionId: React.Dispatch<React.SetStateAction<string | null>>,
) {
  // ---------------------------------------------------------------------------
  // Refs for latest option values (avoid stale closures)
  // ---------------------------------------------------------------------------
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

  // ---------------------------------------------------------------------------
  // Stream state
  // ---------------------------------------------------------------------------
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamingMessageId, setStreamingMessageId] = useState<string | null>(null);

  const streamingSessionIdRef = useRef<string | null>(null);
  const streamingMessageIdRef = useRef<string | null>(null);
  const streamTypewriterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const streamTypewriterQueueRef = useRef("");
  const streamDisplayedTextRef = useRef("");
  const streamReceivedTokenRef = useRef(false);
  const streamReduceMotionRef = useRef(false);
  const stopControllerRef = useRef<AbortController | null>(null);
  const pendingDoneEventRef = useRef<PendingDoneEvent | null>(null);

  // ---------------------------------------------------------------------------
  // Typewriter helpers
  // ---------------------------------------------------------------------------

  function stopStreamingTypewriter() {
    if (streamTypewriterTimerRef.current !== null) {
      clearTimeout(streamTypewriterTimerRef.current);
      streamTypewriterTimerRef.current = null;
    }
  }

  function resetStreamingTypewriter() {
    stopStreamingTypewriter();
    streamTypewriterQueueRef.current = "";
    streamDisplayedTextRef.current = "";
    streamReceivedTokenRef.current = false;
    streamReduceMotionRef.current = getPrefersReducedStreamingMotion();
    pendingDoneEventRef.current = null;
  }

  // ---------------------------------------------------------------------------
  // Message accumulation helpers
  // ---------------------------------------------------------------------------

  function updateStreamingAssistant(
    updater: (current: ChatMessage | null) => ChatMessage,
    targetId?: string | null,
    fallbackId?: string | null,
  ) {
    const candidateIds = [targetId ?? streamingMessageIdRef.current, fallbackId].filter(
      (value): value is string => Boolean(value),
    );

    if (candidateIds.length === 0) {
      return;
    }

    messageHistory.setMessages((current) => {
      let found = false;
      const next = current.map((message) => {
        const matchesId = candidateIds.includes(message.id);
        const matchesPendingAssistant = !matchesId && message.role === "assistant" && message.pending;

        if (!matchesId && !matchesPendingAssistant) {
          return message;
        }

        found = true;
        return updater(message);
      });

      if (!found) {
        next.push(updater(null));
      }

      return next;
    });
  }

  function ensureStreamingAssistant(
    event: Extract<WorkspaceChatStreamEvent, { kind: "answer_start" | "token" | "citations" }>,
  ) {
    const resolvedMessageId = normalizeStreamMessageId(event.message_id);
    const fallbackAssistantId = getAssistantMessageKey(event.message_id);
    const eventMode = event.kind === "answer_start" ? normalizeMessageMode(event.agent_type) : null;

    updateStreamingAssistant(
      (current) => ({
        id:
          current?.id ??
          streamingMessageIdRef.current ??
          (resolvedMessageId !== null ? getAssistantMessageKey(resolvedMessageId) : fallbackAssistantId) ??
          `assistant-${Date.now()}`,
        role: "assistant",
        mode: eventMode ?? current?.mode ?? effectiveChatModeRef.current,
        content: current?.content ?? "",
        answerBlocks: current?.answerBlocks ?? [],
        citations: event.kind === "citations" ? event.citations : current?.citations ?? [],
        degradeTrace: current?.degradeTrace ?? [],
        guarded: current?.guarded ?? false,
        messageId: resolvedMessageId ?? current?.messageId ?? null,
        pending: true,
        sessionId:
          event.kind === "answer_start"
            ? current?.sessionId ?? event.session_id
            : current?.sessionId ?? streamingSessionIdRef.current,
        toolResults: current?.toolResults ?? [],
      }),
      undefined,
      fallbackAssistantId,
    );
  }

  function appendStreamingDisplayText(chunk: string) {
    if (!chunk) {
      return;
    }

    streamDisplayedTextRef.current += chunk;
    updateStreamingAssistant((current) => ({
      id: current?.id ?? streamingMessageIdRef.current ?? `assistant-${Date.now()}`,
      role: "assistant",
      mode: current?.mode ?? effectiveChatModeRef.current,
      content: `${current?.content ?? ""}${chunk}`,
      answerBlocks: current?.answerBlocks ?? [],
      citations: current?.citations ?? [],
      degradeTrace: current?.degradeTrace ?? [],
      guarded: current?.guarded ?? false,
      messageId: current?.messageId ?? null,
      pending: true,
      sessionId: current?.sessionId ?? streamingSessionIdRef.current,
      toolResults: current?.toolResults ?? [],
    }));
  }

  function finalizeStreamingDone(event: PendingDoneEvent) {
    const answer = getAnswerText(event.payload.answer ?? "", event.payload.answer_blocks ?? []);
    const resolvedMessageId = normalizeStreamMessageId(event.message_id);
    const fallbackAssistantId = getAssistantMessageKey(event.message_id);

    updateStreamingAssistant(
      (current) => ({
        id: resolvedMessageId !== null ? getAssistantMessageKey(resolvedMessageId) : current?.id ?? fallbackAssistantId,
        role: "assistant",
        mode: normalizeMessageMode(event.payload.agent_type) ?? current?.mode ?? effectiveChatModeRef.current,
        content: answer || current?.content || "",
        answerBlocks:
          event.payload.answer_blocks && event.payload.answer_blocks.length > 0
            ? event.payload.answer_blocks
            : current?.answerBlocks ?? [],
        citations:
          event.payload.citations && event.payload.citations.length > 0
            ? event.payload.citations
            : current?.citations ?? [],
        degradeTrace: event.payload.degrade_trace ?? [],
        guarded: hasGuardrailIntervention(event.payload.guard_report),
        messageId: resolvedMessageId ?? current?.messageId ?? null,
        pending: false,
        sessionId: event.session_id,
        toolResults: event.payload.tool_results ?? current?.toolResults ?? [],
      }),
      undefined,
      fallbackAssistantId,
    );

    streamingSessionIdRef.current = event.session_id;
    setActiveSessionId(event.session_id);
    onSessionChangeRef.current?.(event.session_id);
    setIsStreaming(false);
    setStreamingMessageId(null);
    streamingMessageIdRef.current = null;
    resetStreamingTypewriter();
  }

  function finalizePendingDoneIfReady() {
    if (streamTypewriterQueueRef.current.length > 0 || !pendingDoneEventRef.current) {
      return;
    }
    finalizeStreamingDone(pendingDoneEventRef.current);
  }

  function flushStreamingTypewriterQueue() {
    streamTypewriterTimerRef.current = null;

    const nextChunk = streamTypewriterQueueRef.current.slice(0, STREAM_TYPEWRITER_CHARS_PER_TICK);
    streamTypewriterQueueRef.current = streamTypewriterQueueRef.current.slice(STREAM_TYPEWRITER_CHARS_PER_TICK);
    appendStreamingDisplayText(nextChunk);

    if (streamTypewriterQueueRef.current.length > 0) {
      scheduleStreamingTypewriter();
      return;
    }

    finalizePendingDoneIfReady();
  }

  function scheduleStreamingTypewriter() {
    if (streamTypewriterTimerRef.current !== null) {
      return;
    }
    streamTypewriterTimerRef.current = setTimeout(flushStreamingTypewriterQueue, STREAM_TYPEWRITER_INTERVAL_MS);
  }

  function enqueueStreamingText(text: string) {
    if (!text) {
      finalizePendingDoneIfReady();
      return;
    }

    if (streamReduceMotionRef.current) {
      appendStreamingDisplayText(text);
      finalizePendingDoneIfReady();
      return;
    }

    streamTypewriterQueueRef.current += text;
    scheduleStreamingTypewriter();
  }

  function shouldDrainTypewriterQueueAfterDone(event: PendingDoneEvent) {
    if (!streamReceivedTokenRef.current || streamReduceMotionRef.current) {
      return false;
    }

    const queuedText = streamTypewriterQueueRef.current;

    if (!queuedText) {
      return false;
    }

    if (queuedText.length > STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE) {
      return false;
    }

    const answer = getStreamingDisplayText(event.payload.answer ?? "", event.payload.answer_blocks ?? []);

    if (!answer) {
      return true;
    }

    const queuedAnswer = `${streamDisplayedTextRef.current}${queuedText}`;

    if (!answer.startsWith(queuedAnswer)) {
      return false;
    }

    return answer.length - queuedAnswer.length <= STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE;
  }

  function handleDoneWithTypewriter(event: PendingDoneEvent) {
    if (!shouldDrainTypewriterQueueAfterDone(event)) {
      finalizeStreamingDone(event);
      return;
    }

    pendingDoneEventRef.current = event;
    scheduleStreamingTypewriter();
  }

  function clearPendingStreamingAssistant() {
    const pendingMessageId = streamingMessageIdRef.current ?? streamingMessageId;

    if (!pendingMessageId) {
      return;
    }

    messageHistory.setMessages((current) =>
      current.map((message) =>
        message.id === pendingMessageId ? { ...message, pending: false } : message,
      ),
    );
  }

  function beginAnswerStreaming(event: Extract<WorkspaceChatStreamEvent, { kind: "answer_start" }>) {
    ensureStreamingAssistant(event);
  }

  // ---------------------------------------------------------------------------
  // Event handler
  // ---------------------------------------------------------------------------

  const handleStreamEvent = useCallback(
    (event: WorkspaceChatStreamEvent) => {
      switch (event.kind) {
        case "start":
          if (event.session_id) {
            streamingSessionIdRef.current = event.session_id;
            setActiveSessionId(event.session_id);
            onSessionChangeRef.current?.(event.session_id);
          }
          break;
        case "activity":
          progressTracker.addActivity(event);
          break;
        case "answer_start":
          if (normalizeMessageMode(event.agent_type) !== "chat") {
            beginAnswerStreaming(event);
          }
          break;
        case "token": {
          const activeProgressMode = progressTracker.modeRef.current;
          if (!activeProgressMode || !isResearchMode(activeProgressMode)) {
            progressTracker.hide();
          }
          ensureStreamingAssistant(event);
          streamReceivedTokenRef.current = true;
          enqueueStreamingText(event.content);
          break;
        }
        case "reasoning_summary_delta":
          progressTracker.addReasoning(event.content);
          break;
        case "citations":
          ensureStreamingAssistant(event);
          break;
        case "done": {
          progressTracker.hide();
          handleDoneWithTypewriter(event);
          break;
        }
        case "error":
          progressTracker.hide();
          resetStreamingTypewriter();
          clearPendingStreamingAssistant();
          setError(event.message);
          setIsStreaming(false);
          setStreamingMessageId(null);
          streamingSessionIdRef.current = null;
          streamingMessageIdRef.current = null;
          break;
        case "trace":
          break;
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [progressTracker, setError, setActiveSessionId, streamingMessageId],
  );

  // ---------------------------------------------------------------------------
  // Send / Stop
  // ---------------------------------------------------------------------------

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
      resetStreamingTypewriter();
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
            },
            handleStreamEvent,
            { signal: controller.signal },
          );
        } catch (submitError) {
          if (submitError instanceof Error && submitError.name === "AbortError") {
            return;
          }
          progressTracker.hide();
          resetStreamingTypewriter();
          clearPendingStreamingAssistant();
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
    [isStreaming, messageHistory.setMessages, progressTracker, setError],
  );

  const stop = useCallback(() => {
    const controller = stopControllerRef.current;
    if (!controller) {
      return;
    }

    stopControllerRef.current = null;
    controller.abort();

    progressTracker.hide();
    resetStreamingTypewriter();
    clearPendingStreamingAssistant();
    setIsStreaming(false);
    setStreamingMessageId(null);
    streamingMessageIdRef.current = null;
  }, [progressTracker, setError]);

  // ---------------------------------------------------------------------------
  // Cleanup on unmount
  // ---------------------------------------------------------------------------

  useEffect(() => {
    return () => {
      resetStreamingTypewriter();
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
    resetStreamingTypewriter,
    streamingSessionIdRef,
    streamingMessageIdRef,
  };
}

// =============================================================================
// Main Hook
// =============================================================================

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

  // Sync activeSessionId when sessionId prop changes
  useEffect(() => {
    setActiveSessionId(sessionId);
  }, [sessionId]);

  // Auto-load session when sessionId or token changes
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
