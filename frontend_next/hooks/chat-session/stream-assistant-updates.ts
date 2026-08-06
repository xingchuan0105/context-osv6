import type { ChatEvent, ChatResponse, Citation } from "../../lib/contracts";
import {
  capabilitiesFromAgentType,
  deriveAgentTypeLabel,
  normalizeCapabilities,
  type WorkspaceCapability,
} from "../../lib/workspace/capabilities";
import { parseStreamCitations } from "../../lib/workspace/stream";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import {
  getAnswerText,
  getAssistantMessageKey,
  hasGuardrailIntervention,
  normalizeMessageMode,
  normalizeStreamMessageId,
  sanitizeAssistantDisplayContent,
} from "./helpers";
import type { MessageHistory } from "./use-message-history";
import type { PendingDoneEvent, UiChatMessage, UiProgressSnapshot } from "./types";

function resolveAssistantCapabilities(
  agentType: string | null | undefined,
  fallbackCaps: WorkspaceCapability[],
  fallbackMode: WorkspaceChatMode,
): WorkspaceCapability[] {
  const fromAgent = capabilitiesFromAgentType(agentType);
  if (fromAgent.length > 0) {
    return fromAgent;
  }
  if (fallbackCaps.length > 0) {
    return normalizeCapabilities(fallbackCaps);
  }
  return capabilitiesFromAgentType(fallbackMode);
}

export type StreamAssistantUpdateDeps = {
  messageHistory: MessageHistory;
  streamingMessageIdRef: React.MutableRefObject<string | null>;
  streamingSessionIdRef: React.MutableRefObject<string | null>;
  effectiveChatModeRef: React.MutableRefObject<WorkspaceChatMode>;
  capabilitiesRef: React.MutableRefObject<WorkspaceCapability[]>;
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
    const eventCaps =
      event.event === "answer_start"
        ? resolveAssistantCapabilities(
            event.agent_type,
            deps.capabilitiesRef.current,
            deps.effectiveChatModeRef.current,
          )
        : null;

    updateStreamingAssistant(
      (current) => {
        const capabilities =
          eventCaps ??
          current?.capabilities ??
          resolveAssistantCapabilities(
            null,
            deps.capabilitiesRef.current,
            deps.effectiveChatModeRef.current,
          );
        const mode =
          eventMode ??
          current?.mode ??
          (capabilities.length > 0
            ? deriveAgentTypeLabel(capabilities)
            : deps.effectiveChatModeRef.current);
        return {
          id:
            current?.id ??
            deps.streamingMessageIdRef.current ??
            (resolvedMessageId !== null
              ? getAssistantMessageKey(resolvedMessageId)
              : fallbackAssistantId) ??
            `assistant-${Date.now()}`,
          role: "assistant",
          mode,
          capabilities,
          content: current?.content ?? "",
          answerBlocks: current?.answerBlocks ?? [],
          citations:
            event.event === "citations"
              ? parseStreamCitations(event.citations)
              : current?.citations ?? [],
          degradeTrace: current?.degradeTrace ?? [],
          guarded: current?.guarded ?? false,
          messageId: resolvedMessageId ?? current?.messageId ?? null,
          pending: true,
          sessionId:
            event.event === "answer_start"
              ? current?.sessionId ?? event.session_id
              : current?.sessionId ?? deps.streamingSessionIdRef.current,
          toolResults: current?.toolResults ?? [],
          progress: current?.progress ?? null,
        };
      },
      undefined,
      fallbackAssistantId,
    );
  }

  function appendStreamingDisplayText(chunk: string) {
    if (!chunk) {
      return;
    }

    updateStreamingAssistant((current) => {
      const capabilities =
        current?.capabilities ??
        resolveAssistantCapabilities(
          null,
          deps.capabilitiesRef.current,
          deps.effectiveChatModeRef.current,
        );
      return {
        id: current?.id ?? deps.streamingMessageIdRef.current ?? `assistant-${Date.now()}`,
        role: "assistant",
        mode:
          current?.mode ??
          (capabilities.length > 0
            ? deriveAgentTypeLabel(capabilities)
            : deps.effectiveChatModeRef.current),
        capabilities,
        content: `${current?.content ?? ""}${chunk}`,
        answerBlocks: current?.answerBlocks ?? [],
        citations: current?.citations ?? [],
        degradeTrace: current?.degradeTrace ?? [],
        guarded: current?.guarded ?? false,
        messageId: current?.messageId ?? null,
        pending: true,
        sessionId: current?.sessionId ?? deps.streamingSessionIdRef.current,
        toolResults: current?.toolResults ?? [],
        progress: current?.progress ?? null,
      };
    });
  }

  function finalizeStreamingDone(
    event: PendingDoneEvent,
    progressSnapshot?: UiProgressSnapshot | null,
  ) {
    const payload = event.payload as ChatResponse;
    // Done is authoritative for the main bubble. Never fall back to
    // `current?.content` (live token buffer) — that used to keep retrieve-phase
    // drafts (sandbox code) on screen when the final answer was empty/stripped.
    const answer = getAnswerText(payload.answer ?? "", payload.answer_blocks ?? []);
    const resolvedMessageId = normalizeStreamMessageId(event.message_id);
    const fallbackAssistantId = getAssistantMessageKey(event.message_id);
    updateStreamingAssistant(
      (current) => {
        const capabilities = resolveAssistantCapabilities(
          payload.agent_type,
          current?.capabilities ?? deps.capabilitiesRef.current,
          deps.effectiveChatModeRef.current,
        );
        return {
          id:
            resolvedMessageId !== null
              ? getAssistantMessageKey(resolvedMessageId)
              : current?.id ?? fallbackAssistantId,
          role: "assistant",
          mode:
            normalizeMessageMode(payload.agent_type) ??
            current?.mode ??
            (capabilities.length > 0
              ? deriveAgentTypeLabel(capabilities)
              : deps.effectiveChatModeRef.current),
          capabilities,
          content: answer,
          answerBlocks:
            payload.answer_blocks && payload.answer_blocks.length > 0
              ? payload.answer_blocks
              : [],
          citations:
            payload.citations && payload.citations.length > 0
              ? payload.citations
              : [],
          degradeTrace: payload.degrade_trace ?? [],
          guarded: hasGuardrailIntervention(payload.guard_report),
          messageId: resolvedMessageId ?? current?.messageId ?? null,
          pending: false,
          sessionId: event.session_id,
          toolResults: payload.tool_results ?? current?.toolResults ?? [],
          progress: progressSnapshot ?? current?.progress ?? null,
        };
      },
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

    // Keep an assistant slot after errors/abort so the transcript never ends on a
    // bare user query. Empty content is left for the error banner; non-empty keeps text.
    deps.messageHistory.setMessages((current) =>
      current.map((message) =>
        message.id === pendingMessageId
          ? {
              ...message,
              pending: false,
              content:
                message.content.trim().length > 0
                  ? message.content
                  : message.content /* may stay empty; UI shows stream error banner */,
            }
          : message,
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
