import type { WorkspaceChatMessage } from "../../lib/workspace/client";
import {
  capabilitiesFromAgentType,
  deriveAgentTypeLabel,
  normalizeCapabilities,
  type WorkspaceCapability,
} from "../../lib/workspace/capabilities";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import type { AnswerBlock } from "../../lib/workspace/stream";
import { progressSnapshotFromTurnMetadata } from "./progress-i18n";
import type { ProgressEntry, UiChatMessage } from "./types";

export const STREAM_TYPEWRITER_CHARS_PER_TICK = 8;
export const STREAM_TYPEWRITER_INTERVAL_MS = 16;
export const STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE = 80;

export function normalizeMessageMode(mode: string | null | undefined): WorkspaceChatMode | null {
  if (mode === "general" || mode === "chat") {
    return "chat";
  }
  if (mode === "rag" || mode === "search" || mode === "write" || mode === "rag+search") {
    return mode;
  }
  return null;
}

/** Prefer turn_metadata.capabilities; fall back to legacy agent_id / agent_type. */
export function normalizeMessageCapabilities(
  message: Pick<WorkspaceChatMessage, "agent_id" | "turn_metadata">,
): WorkspaceCapability[] {
  const meta = message.turn_metadata;
  if (meta && typeof meta === "object" && "capabilities" in meta) {
    return normalizeCapabilities(meta.capabilities);
  }
  return capabilitiesFromAgentType(message.agent_id);
}

/**
 * Hide non-user-facing payload that sometimes leaks into assistant `content`:
 * - sandbox / tool code fences and host observation shells
 * - whole-message tool JSON dumps (e.g. doc_profile)
 */
export function sanitizeAssistantDisplayContent(content: string): string {
  if (!content) {
    return content;
  }
  let s = content
    // Markdown fences (often codegen that the model echoed into the final turn)
    .replace(/```[\w-]*\r?\n[\s\S]*?```/g, "")
    // Inline HTML-ish code / host observation shells
    .replace(/<code\b[^>]*>[\s\S]*?<\/code>/gi, "")
    .replace(/<code_execution_result\b[^>]*>[\s\S]*?<\/code_execution_result>/gi, "")
    .replace(/<retrieval_summary\b[^>]*>[\s\S]*?<\/retrieval_summary>/gi, "")
    .replace(/<loop_budget\b[^>]*>[\s\S]*?<\/loop_budget>/gi, "")
    // Common tool-call XML shells
    .replace(/<\/?tool_call\b[^>]*>/gi, "")
    .replace(/<\/?function_call\b[^>]*>/gi, "");

  // Collapse leftover blank runs after stripping
  s = s.replace(/\n{3,}/g, "\n\n").trim();

  const trimmed = s.trim();
  if (!trimmed) {
    return "";
  }
  if (!(trimmed.startsWith("{") || trimmed.startsWith("["))) {
    return s;
  }
  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (Array.isArray(parsed)) {
      const looksLikeDocProfile = parsed.every(
        (item) =>
          item &&
          typeof item === "object" &&
          ("doc_id" in item || "name" in item || "chunk_id" in item),
      );
      if (looksLikeDocProfile) {
        return "";
      }
    }
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const keys = Object.keys(parsed as object);
      if (keys.some((k) => k === "doc_id" || k === "tool" || k === "chunks")) {
        return "";
      }
    }
  } catch {
    return s;
  }
  return s;
}

export function mapTranscriptMessage(
  message: WorkspaceChatMessage,
  locale: "zh-CN" | "en" = "zh-CN",
): UiChatMessage {
  const rawContent = message.content ?? "";
  const content =
    message.role === "assistant" ? sanitizeAssistantDisplayContent(rawContent) : rawContent;
  // Server is source of truth: progress lives on assistant turn_metadata (cross-device).
  const progress =
    message.role === "assistant"
      ? progressSnapshotFromTurnMetadata(locale, message.turn_metadata)
      : null;
  const capabilities =
    message.role === "assistant" ? normalizeMessageCapabilities(message) : [];
  const mode =
    message.role === "assistant"
      ? capabilities.length > 0
        ? deriveAgentTypeLabel(capabilities)
        : normalizeMessageMode(message.agent_id)
      : null;
  return {
    id: String(message.id),
    role: message.role === "assistant" ? "assistant" : "user",
    mode,
    capabilities,
    content,
    answerBlocks: message.answer_blocks ?? [],
    citations: message.citations ?? [],
    degradeTrace: [],
    guarded: false,
    messageId: message.id,
    pending: false,
    sessionId: message.session_id,
    toolResults: message.tool_results ?? [],
    progress,
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
  const safeContent = sanitizeAssistantDisplayContent(content);
  return safeContent.trim().length > 0 ? safeContent : blockText;
}

export function getStreamingDisplayText(content: string, blocks: AnswerBlock[]) {
  const blockText = getAnswerBlockText(blocks);
  const safeContent = sanitizeAssistantDisplayContent(content);
  return blockText || safeContent;
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
  return mode === "rag" || mode === "search" || mode === "rag+search";
}

export function getInitialProgressEntry(locale: "zh-CN" | "en", mode: WorkspaceChatMode): ProgressEntry {
  if (locale === "zh-CN") {
    if (mode === "rag" || mode === "rag+search") {
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
    if (mode === "search") {
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
    if (mode === "write") {
      return {
        id: "progress-initial",
        phase: "planning",
        title: "正在准备写作流程",
        detail: "系统正在组织结构与素材。",
        counts: {},
        sourcesPreview: [],
        timestamp: null,
      };
    }
    return {
      id: "progress-initial",
      phase: "thinking",
      title: "正在思考",
      detail: null,
      counts: {},
      sourcesPreview: [],
      timestamp: null,
    };
  }
  if (mode === "rag" || mode === "rag+search") {
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
  if (mode === "search") {
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
  if (mode === "write") {
    return {
      id: "progress-initial",
      phase: "planning",
      title: "Preparing the writing flow",
      detail: "Organizing structure and source material.",
      counts: {},
      sourcesPreview: [],
      timestamp: null,
    };
  }
  return {
    id: "progress-initial",
    phase: "thinking",
    title: "Thinking",
    detail: null,
    counts: {},
    sourcesPreview: [],
    timestamp: null,
  };
}
