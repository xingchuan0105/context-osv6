import type { WorkspaceChatMessage } from "../../lib/workspace/client";
import {
  capabilitiesFromAgentType,
  deriveAgentTypeLabel,
  normalizeCapabilities,
  type WorkspaceCapability,
} from "../../lib/workspace/capabilities";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import { parseStreamCitations, type AnswerBlock, type Citation } from "../../lib/workspace/stream";
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
 * Structural cleanup of assistant `content` for display only.
 *
 * Strips **shapes** (code fences, host observation tags, bare `client.*` tokens,
 * whole-message unfenced code drafts, tool JSON). Does **not** encode product
 * policy, Chinese confession heuristics, or named tool catalogues — those live
 * in prompts / host final-answer rules.
 */
export function sanitizeAssistantDisplayContent(content: string): string {
  if (!content) {
    return content;
  }
  // Whole-message bare code (no fences) — blank rather than half-strip debris.
  if (isUnfencedCodeShaped(content)) {
    return "";
  }
  let s = content
    // Fenced blocks: 2–3 backticks (models sometimes omit the third)
    .replace(/`{2,3}[\w-]*\r?\n[\s\S]*?`{2,3}/g, "")
    .replace(/`{2,3}[\w-]*\r?\n[\s\S]*$/g, "")
    // Host observation / code shells (tag names only — no prose policy)
    .replace(/<code\b[^>]*>[\s\S]*?<\/code>/gi, "")
    .replace(/<code_execution_result\b[^>]*>[\s\S]*?<\/code_execution_result>/gi, "")
    .replace(/<retrieval_summary\b[^>]*>[\s\S]*?<\/retrieval_summary>/gi, "")
    .replace(/<loop_budget\b[^>]*>[\s\S]*?<\/loop_budget>/gi, "")
    .replace(/<\/?tool_call\b[^>]*>/gi, "")
    .replace(/<\/?function_call\b[^>]*>/gi, "")
    // Structural sandbox line shapes (mixed prose+code leftovers)
    .replace(/^[ \t]*import\s+\w+[ \t]*$/gim, "")
    .replace(/^[ \t]*from\s+\S+[ \t]+import\b.*$/gim, "")
    .replace(/^[ \t]*.*\bawait\s+client\.\w+\s*\([^)]*\)[ \t]*$/gim, "")
    .replace(/^[ \t]*print\s*\(.*\)[ \t]*$/gim, "")
    // Inline SDK surface tokens (any method id, not a fixed product list)
    .replace(/`client\.\w+\([^`]*\)`/g, "")
    .replace(/\bclient\.\w+\b/g, "");

  s = s.replace(/[ \t]+\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
  // After strip, residual may still be code-shaped (assignments / if: left behind).
  if (isUnfencedCodeShaped(s)) {
    return "";
  }
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

/** Line-shape classifier mirrored by host `is_unfenced_code_shaped` (structure only). */
function isUnfencedCodeShaped(text: string): boolean {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (lines.length === 0) {
    return false;
  }
  const codeN = lines.filter((l) => lineLooksLikeCode(l)).length;
  const allCode = codeN === lines.length;
  if (allCode && lines.length >= 2) {
    return true;
  }
  if (allCode && lines.length === 1 && lineIsStrongCode(lines[0])) {
    return true;
  }
  if (lines.length >= 3 && codeN * 4 >= lines.length * 3 && lines.some(lineIsStrongCode)) {
    return true;
  }
  return false;
}

function lineIsStrongCode(line: string): boolean {
  const t = line.trim();
  // Prefix / whole-statement shapes only — mid-prose `client.foo` is not a code line.
  return (
    t.startsWith("await ") ||
    t.startsWith("import ") ||
    t.startsWith("from ") ||
    t.startsWith("print(") ||
    t.startsWith("def ") ||
    t.startsWith("async def ") ||
    t.startsWith("async ") ||
    t.startsWith("class ") ||
    t.startsWith("function ") ||
    t.startsWith("const ") ||
    t.startsWith("let ") ||
    t.startsWith("var ") ||
    t.includes(" = await ") ||
    t.includes("=await ")
  );
}

function lineLooksLikeCode(line: string): boolean {
  const t = line.trim();
  if (!t) {
    return false;
  }
  if (t.startsWith("#") || t.startsWith("//") || t.startsWith("/*")) {
    return true;
  }
  if (lineIsStrongCode(t)) {
    return true;
  }
  const control =
    /^(if\s|elif\s|else:|else\s+if|for\s|while\s|try:|try\s|except|finally:|with\s|def\s|class\s|async\s|return\s|raise\s|yield\s|assert\s|pass\b|break\b|continue\b|match\s|case\s|lambda\s)/;
  if (control.test(t)) {
    return true;
  }
  if (t.endsWith(":") && t.length > 1 && !t.startsWith("http")) {
    return true;
  }
  if (looksLikeAssignment(t)) {
    return true;
  }
  if (looksLikeCallStatement(t)) {
    return true;
  }
  return false;
}

function looksLikeAssignment(line: string): boolean {
  const eq = line.indexOf("=");
  if (eq < 0) {
    return false;
  }
  if (line[eq + 1] === "=") {
    return false;
  }
  if (eq > 0 && ["!", "<", ">"].includes(line[eq - 1]!)) {
    return false;
  }
  const lhs = line.slice(0, eq).trim();
  if (!lhs) {
    return false;
  }
  return /^[A-Za-z0-9_[\]"'./]+$/.test(lhs);
}

function looksLikeCallStatement(line: string): boolean {
  const t = line.trim();
  if (!t.endsWith(")") || !t.includes("(")) {
    return false;
  }
  if (t.includes("。") || t.includes(". ")) {
    return false;
  }
  const open = t.indexOf("(");
  const callee = t.slice(0, open).trim();
  return callee.length > 0 && /^[A-Za-z0-9_.]+$/.test(callee);
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

/** Done strips `citations[].content`; keep text from the earlier citations SSE event. */
export function mergeCitationsPreservingContent(
  payloadCitations: unknown,
  current: Citation[],
): Citation[] {
  const incoming = parseStreamCitations(payloadCitations);
  if (incoming.length === 0) {
    return current;
  }
  const priorByKey = new Map(
    current.map((citation) => [citation.chunk_id?.trim() || `id:${citation.citation_id}`, citation]),
  );
  return incoming.map((citation) => {
    const prior = priorByKey.get(citation.chunk_id?.trim() || `id:${citation.citation_id}`);
    if (!prior) {
      return citation;
    }
    return {
      ...citation,
      content: citation.content?.trim() ? citation.content : prior.content,
      preview: citation.preview?.trim() ? citation.preview : prior.preview,
    };
  });
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
