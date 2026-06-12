import type { WorkspaceChatMessage } from "../../lib/workspace/client";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { AnswerBlock } from "../../lib/workspace/stream";
import type { ProgressEntry, UiChatMessage } from "./types";

export const STREAM_TYPEWRITER_CHARS_PER_TICK = 8;
export const STREAM_TYPEWRITER_INTERVAL_MS = 16;
export const STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE = 80;

export function normalizeMessageMode(mode: string | null | undefined): WorkspaceChatMode | null {
  if (mode === "general" || mode === "chat") {
    return "chat";
  }
  if (mode === "rag" || mode === "search") {
    return mode;
  }
  return null;
}

export function mapTranscriptMessage(message: WorkspaceChatMessage): UiChatMessage {
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

export function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
}

export function getAnswerText(content: string, blocks: AnswerBlock[]) {
  const blockText = getAnswerBlockText(blocks);
  return content.trim().length > 0 ? content : blockText;
}

export function getStreamingDisplayText(content: string, blocks: AnswerBlock[]) {
  const blockText = getAnswerBlockText(blocks);
  return blockText || content;
}

export function getPrefersReducedStreamingMotion() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function hasGuardrailIntervention(guardReport: unknown) {
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

export function normalizeStreamMessageId(messageId: number) {
  return messageId > 0 ? messageId : null;
}

export function getAssistantMessageKey(messageId: number) {
  return `assistant-${messageId}`;
}

export function isResearchMode(mode: WorkspaceChatMode) {
  return mode === "rag" || mode === "search";
}

export function getInitialProgressEntry(locale: "zh-CN" | "en", mode: WorkspaceChatMode): ProgressEntry {
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
