import type { ChatEvent } from "../../lib/contracts";
import { isResearchMode, normalizeMessageMode } from "./helpers";
import type { ProgressTracker } from "./use-progress-tracker";
import type { PendingDoneEvent, UseChatSessionOptions } from "./types";

/** Dependencies shared by per-event stream reducers (refs + callbacks from `useChatStream`). */
export type StreamEventHandlerDeps = {
  progressTracker: ProgressTracker;
  setError: React.Dispatch<React.SetStateAction<string>>;
  setActiveSessionId: React.Dispatch<React.SetStateAction<string | null>>;
  setIsStreaming: React.Dispatch<React.SetStateAction<boolean>>;
  setStreamingMessageId: React.Dispatch<React.SetStateAction<string | null>>;
  streamingSessionIdRef: React.MutableRefObject<string | null>;
  streamingMessageIdRef: React.MutableRefObject<string | null>;
  onSessionChangeRef: React.MutableRefObject<UseChatSessionOptions["onSessionChange"] | undefined>;
  streamingMessageId: string | null;
  beginAnswerStreaming: (event: Extract<ChatEvent, { event: "answer_start" }>) => void;
  ensureStreamingAssistant: (
    event: Extract<ChatEvent, { event: "answer_start" | "token" | "citations" }>,
  ) => void;
  markTokenReceived: () => void;
  enqueueStreamingText: (text: string) => void;
  handleDoneWithTypewriter: (event: PendingDoneEvent) => void;
  resetStreamingTypewriter: () => void;
  clearPendingStreamingAssistant: () => void;
};

export function handleStartEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "start" }>,
) {
  if (!event.session_id) {
    return;
  }

  deps.streamingSessionIdRef.current = event.session_id;
  deps.setActiveSessionId(event.session_id);
  deps.onSessionChangeRef.current?.(event.session_id);
}

export function handleActivityEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "activity" }>,
) {
  deps.progressTracker.addActivity(event);
}

export function handleAnswerStartEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "answer_start" }>,
) {
  if (normalizeMessageMode(event.agent_type) === "chat") {
    return;
  }

  deps.beginAnswerStreaming(event);
}

export function handleTokenEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "token" }>,
) {
  const activeProgressMode = deps.progressTracker.modeRef.current;

  if (!activeProgressMode || !isResearchMode(activeProgressMode)) {
    deps.progressTracker.hide();
  }

  deps.ensureStreamingAssistant(event);
  deps.markTokenReceived();
  deps.enqueueStreamingText(event.content);
}

export function handleReasoningSummaryDeltaEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "reasoning_summary_delta" }>,
) {
  deps.progressTracker.addReasoning(event.content);
}

export function handleCitationsEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "citations" }>,
) {
  deps.ensureStreamingAssistant(event);
}

export function handleDoneEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "done" }>,
) {
  deps.progressTracker.hide();
  deps.handleDoneWithTypewriter(event as PendingDoneEvent);
}

export function handleErrorEvent(
  deps: StreamEventHandlerDeps,
  event: Extract<ChatEvent, { event: "error" }>,
) {
  deps.progressTracker.hide();
  deps.resetStreamingTypewriter();
  deps.clearPendingStreamingAssistant();
  deps.setError(event.message);
  deps.setIsStreaming(false);
  deps.setStreamingMessageId(null);
  deps.streamingSessionIdRef.current = null;
  deps.streamingMessageIdRef.current = null;
}

export function handleTraceEvent(
  _deps: StreamEventHandlerDeps,
  _event: Extract<ChatEvent, { event: "trace" }>,
) {
  // Trace events are diagnostic only; UI ignores them today.
}

export function dispatchStreamEvent(deps: StreamEventHandlerDeps, event: ChatEvent) {
  switch (event.event) {
    case "start":
      handleStartEvent(deps, event);
      break;
    case "activity":
      handleActivityEvent(deps, event);
      break;
    case "answer_start":
      handleAnswerStartEvent(deps, event);
      break;
    case "token":
      handleTokenEvent(deps, event);
      break;
    case "reasoning_summary_delta":
      handleReasoningSummaryDeltaEvent(deps, event);
      break;
    case "citations":
      handleCitationsEvent(deps, event);
      break;
    case "done":
      handleDoneEvent(deps, event);
      break;
    case "error":
      handleErrorEvent(deps, event);
      break;
    case "trace":
      handleTraceEvent(deps, event);
      break;
  }
}
