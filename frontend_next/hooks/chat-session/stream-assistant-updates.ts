import type { ChatEvent, ChatResponse, Citation } from "../../lib/contracts";
import { parseStreamCitations } from "../../lib/workspace/stream";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import {
  getAnswerText,
  getAssistantMessageKey,
  hasGuardrailIntervention,
  normalizeMessageMode,
  normalizeStreamMessageId,
} from "./helpers";
import type { MessageHistory } from "./use-message-history";
import type { PendingDoneEvent, UiChatMessage } from "./types";

export type StreamAssistantUpdateDeps = {
  messageHistory: MessageHistory;
  streamingMessageIdRef: React.MutableRefObject<string | null>;
  streamingSessionIdRef: React.MutableRefObject<string | null>;
  effectiveChatModeRef: React.MutableRefObject<WorkspaceChatMode>;
  setActiveSessionId: React.Dispatch<React.SetStateAction<string | null>>;
  onSessionChangeRef: React.MutableRefObject<((sessionId: string | null) => void) | undefined>;
  setIsStreaming: React.Dispatch<React.SetStateAction<boolean>>;
  setStreamingMessageId: React.Dispatch<React.SetStateAction<string | null>>;
  resetStreamingTypewriter: () => void;
  streamingMessageId: string | null;
};

export function createStreamAssistantUpdates(deps: StreamAssistantUpdateDeps) {
  function updateStreamingAssistant(
    updater: (current: UiChatMessage | null) => UiChatMessage,
    targetId?: string | null,
    fallbackId?: string | null,
  ) {
    const candidateIds = [targetId ?? deps.streamingMessageIdRef.current, fallbackId].filter(
      (value): value is string => Boolean(value),
    );

    if (candidateIds.length === 0) {
      return;
    }

    deps.messageHistory.setMessages((current) => {
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
    event: Extract<ChatEvent, { event: "answer_start" | "token" | "citations" }>,
  ) {
    const resolvedMessageId = normalizeStreamMessageId(event.message_id);
    const fallbackAssistantId = getAssistantMessageKey(event.message_id);
    const eventMode = event.event === "answer_start" ? normalizeMessageMode(event.agent_type) : null;

    updateStreamingAssistant(
      (current) => ({
        id:
          current?.id ??
          deps.streamingMessageIdRef.current ??
          (resolvedMessageId !== null ? getAssistantMessageKey(resolvedMessageId) : fallbackAssistantId) ??
          `assistant-${Date.now()}`,
        role: "assistant",
        mode: eventMode ?? current?.mode ?? deps.effectiveChatModeRef.current,
        content: current?.content ?? "",
        answerBlocks: current?.answerBlocks ?? [],
        citations:
          event.event === "citations" ? parseStreamCitations(event.citations) : current?.citations ?? [],
        degradeTrace: current?.degradeTrace ?? [],
        guarded: current?.guarded ?? false,
        messageId: resolvedMessageId ?? current?.messageId ?? null,
        pending: true,
        sessionId:
          event.event === "answer_start"
            ? current?.sessionId ?? event.session_id
            : current?.sessionId ?? deps.streamingSessionIdRef.current,
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

    updateStreamingAssistant((current) => ({
      id: current?.id ?? deps.streamingMessageIdRef.current ?? `assistant-${Date.now()}`,
      role: "assistant",
      mode: current?.mode ?? deps.effectiveChatModeRef.current,
      content: `${current?.content ?? ""}${chunk}`,
      answerBlocks: current?.answerBlocks ?? [],
      citations: current?.citations ?? [],
      degradeTrace: current?.degradeTrace ?? [],
      guarded: current?.guarded ?? false,
      messageId: current?.messageId ?? null,
      pending: true,
      sessionId: current?.sessionId ?? deps.streamingSessionIdRef.current,
      toolResults: current?.toolResults ?? [],
    }));
  }

  function finalizeStreamingDone(event: PendingDoneEvent) {
    const payload = event.payload as ChatResponse;
    const answer = getAnswerText(payload.answer ?? "", payload.answer_blocks ?? []);
    const resolvedMessageId = normalizeStreamMessageId(event.message_id);
    const fallbackAssistantId = getAssistantMessageKey(event.message_id);

    updateStreamingAssistant(
      (current) => ({
        id: resolvedMessageId !== null ? getAssistantMessageKey(resolvedMessageId) : current?.id ?? fallbackAssistantId,
        role: "assistant",
        mode: normalizeMessageMode(payload.agent_type) ?? current?.mode ?? deps.effectiveChatModeRef.current,
        content: getAnswerText(answer || current?.content || "", payload.answer_blocks ?? current?.answerBlocks ?? []),
        answerBlocks:
          payload.answer_blocks && payload.answer_blocks.length > 0
            ? payload.answer_blocks
            : current?.answerBlocks ?? [],
        citations:
          payload.citations && payload.citations.length > 0
            ? payload.citations
            : current?.citations ?? [],
        degradeTrace: payload.degrade_trace ?? [],
        guarded: hasGuardrailIntervention(payload.guard_report),
        messageId: resolvedMessageId ?? current?.messageId ?? null,
        pending: false,
        sessionId: event.session_id,
        toolResults: payload.tool_results ?? current?.toolResults ?? [],
      }),
      undefined,
      fallbackAssistantId,
    );

    deps.streamingSessionIdRef.current = event.session_id;
    deps.setActiveSessionId(event.session_id);
    deps.onSessionChangeRef.current?.(event.session_id);
    deps.setIsStreaming(false);
    deps.setStreamingMessageId(null);
    deps.streamingMessageIdRef.current = null;
    deps.resetStreamingTypewriter();
  }

  function clearPendingStreamingAssistant() {
    const pendingMessageId = deps.streamingMessageIdRef.current ?? deps.streamingMessageId;

    if (!pendingMessageId) {
      return;
    }

    deps.messageHistory.setMessages((current) =>
      current.map((message) =>
        message.id === pendingMessageId ? { ...message, pending: false } : message,
      ),
    );
  }

  function beginAnswerStreaming(event: Extract<ChatEvent, { event: "answer_start" }>) {
    ensureStreamingAssistant(event);
  }

  return {
    updateStreamingAssistant,
    ensureStreamingAssistant,
    appendStreamingDisplayText,
    finalizeStreamingDone,
    clearPendingStreamingAssistant,
    beginAnswerStreaming,
  };
}

export type StreamAssistantUpdates = ReturnType<typeof createStreamAssistantUpdates>;
