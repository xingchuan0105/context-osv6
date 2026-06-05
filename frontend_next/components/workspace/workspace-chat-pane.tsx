"use client";

import {
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  listWorkspaceSessionMessages,
  submitWorkspaceMessageFeedback,
  type WorkspaceChatMessage,
} from "../../lib/workspace/client";
import type {
  WorkspaceCitationAnchor,
  WorkspaceCitationRequest,
  WorkspaceWebSourcesRequest,
  WebSource,
} from "../../lib/workspace/model";
import {
  resolveWorkspaceChatMode,
  useWorkspaceUi,
  type WorkspaceChatMode,
} from "../../lib/workspace/ui-store";
import {
  type ProgressSourcePreview,
  streamWorkspaceChat,
  type AnswerBlock,
  type Citation,
  type DegradeTraceItem,
  type ToolResult,
  type WorkspaceChatStreamEvent,
} from "../../lib/workspace/stream";
import { markdownToInlineHtml, markdownToRichTextHtml } from "./workspace-note-rich-text";

import styles from "./workspace-chat.module.css";

type PaneMessage = {
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
  feedbackRating?: "up" | "down" | null;
  toolResults: ToolResult[];
};

type ProgressEntry = {
  id: string;
  phase: string;
  title: string;
  detail: string | null;
  counts: Record<string, number>;
  sourcesPreview: ProgressSourcePreview[];
  timestamp: string | null;
};

type PendingDoneEvent = Extract<WorkspaceChatStreamEvent, { kind: "done" }>;

type WorkspaceChatPaneProps = {
  workspaceId: string;
  sessionId: string | null;
  selectedSourceIds: string[];
  onSessionActivity?: () => void;
  onSessionChange?: (sessionId: string | null) => void;
  onFocusSource?: (sourceId: string | null) => void;
  onSelectCitation?: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources?: (request: WorkspaceWebSourcesRequest) => void;
};

const CHAT_MODE_ORDER: WorkspaceChatMode[] = ["rag", "search", "chat"];
const MIN_COMPOSER_TEXTAREA_HEIGHT = 52;
const AUTO_COMPOSER_TEXTAREA_MAX_HEIGHT = 192;
const MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT = 360;
const STREAM_TYPEWRITER_CHARS_PER_TICK = 8;
const STREAM_TYPEWRITER_INTERVAL_MS = 16;
const STREAM_TYPEWRITER_MAX_DRAIN_CHARS_AFTER_DONE = 80;

function normalizeMessageMode(mode: string | null | undefined): WorkspaceChatMode | null {
  if (mode === "general" || mode === "chat") {
    return "chat";
  }

  if (mode === "rag" || mode === "search") {
    return mode;
  }

  return null;
}

function mapTranscriptMessage(message: WorkspaceChatMessage): PaneMessage {
  return {
    id: String(message.id),
    role: message.role === "assistant" ? "assistant" : "user",
    mode: message.role === "assistant" ? normalizeMessageMode(message.agent_id) : null,
    content: message.content,
    answerBlocks: message.answer_blocks,
    citations: message.citations,
    degradeTrace: [],
    guarded: false,
    messageId: message.id,
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

function getCitationLabel(citation: Citation, index: number) {
  return citation.doc_name.trim().length > 0 ? citation.doc_name : `Source ${index + 1}`;
}

function getCitationDisplayId(citation: Citation, index: number) {
  return String(citation.citation_id > 0 ? citation.citation_id : index + 1);
}

function getCitationPageText(locale: "zh-CN" | "en", page: number | null | undefined) {
  if (page === null || page === undefined) {
    return "";
  }

  return locale === "zh-CN" ? `第 ${page} 页` : `p. ${page}`;
}

function getCitationAnchorRect(target: HTMLElement): WorkspaceCitationAnchor {
  const rect = target.getBoundingClientRect();

  return {
    top: rect.top,
    left: rect.left,
    right: rect.right,
    bottom: rect.bottom,
    width: rect.width,
    height: rect.height,
  };
}

function escapeHtmlAttribute(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function getInlineCitationAriaLabel(
  locale: "zh-CN" | "en",
  citation: Citation,
  index: number,
) {
  const displayId = getCitationDisplayId(citation, index);
  const label = getCitationLabel(citation, index);
  const pageLabel = getCitationPageText(locale, citation.page);

  if (locale === "zh-CN") {
    return pageLabel ? `引用 ${displayId}：${label}，${pageLabel}` : `引用 ${displayId}：${label}`;
  }

  return pageLabel
    ? `Citation ${displayId}: ${label}, ${pageLabel}`
    : `Citation ${displayId}: ${label}`;
}

function findCitationByChunkId(citations: Citation[], chunkId: string) {
  const normalizedChunkId = chunkId.trim();

  if (!normalizedChunkId) {
    return null;
  }

  return (
    citations.find((citation) => citation.chunk_id?.trim() === normalizedChunkId) ?? null
  );
}

function findCitationByDisplayId(citations: Citation[], displayId: string) {
  return (
    citations.find((citation, index) => getCitationDisplayId(citation, index) === displayId) ??
    null
  );
}

function findCitationIndex(citations: Citation[], target: Citation) {
  return citations.findIndex(
    (citation) =>
      citation === target ||
      (citation.citation_id === target.citation_id &&
        citation.chunk_id === target.chunk_id &&
        citation.doc_id === target.doc_id),
  );
}

function dedupeCitations(citations: Array<Citation | null>) {
  const seen = new Set<string>();

  return citations.filter((citation): citation is Citation => {
    if (!citation) {
      return false;
    }

    const key = `${citation.citation_id}:${citation.chunk_id ?? ""}:${citation.doc_id}`;

    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

function getCitationUrl(citation: Citation): string | null {
  const locatorUrl = citation.source_locator?.url?.trim();
  if (locatorUrl) {
    return locatorUrl;
  }

  const docId = citation.doc_id.trim();
  if (/^https?:\/\//i.test(docId)) {
    return docId;
  }

  return null;
}

function hasOnlyTextAnswerBlocks(blocks: AnswerBlock[]) {
  return (
    blocks.length > 0 &&
    blocks.every((block) => block.type === "text" && block.citations.length === 0)
  );
}

function collectWebSources(citations: Citation[]): WebSource[] {
  const seen = new Set<string>();

  return citations
    .filter((citation) => {
      const url = getCitationUrl(citation);
      if (!url) {
        return false;
      }

      if (seen.has(url)) {
        return false;
      }

      seen.add(url);
      return true;
    })
    .map((citation) => {
      const url = getCitationUrl(citation)!;
      return {
        title: citation.doc_name?.trim() || url,
        url,
        snippet: citation.preview?.trim() || "",
      };
    });
}

function citationToWebSource(citation: Citation): WebSource | null {
  const url = getCitationUrl(citation);

  if (!url) {
    return null;
  }

  return {
    title: citation.doc_name?.trim() || url,
    url,
    snippet: citation.preview?.trim() || "",
  };
}

function hasRenderedCitationMarkup(content: string) {
  return /\[\[image:\d+\]\]|\[\[\d+\]\]|\[\d+\]/u.test(content);
}

type RichMarkdownCitationToken = {
  citation: Citation;
  token: string;
};

function markdownToRichTextHtmlWithCitationButtons(
  markdown: string,
  citations: Citation[],
  locale: "zh-CN" | "en",
) {
  const citationTokens: RichMarkdownCitationToken[] = [];
  const tokenizedMarkdown = markdown.replace(
    /\[\[(\d+)\]\]|\[(?:web:|citation:)?\s*(\d+)\]/gu,
    (marker, bracketedId: string | undefined, prefixedId: string | undefined) => {
      const displayId = bracketedId ?? prefixedId ?? "";
      const citation = findCitationByDisplayId(citations, displayId);

      if (!citation) {
        return marker;
      }

      const token = `CITATIONTOKEN${citationTokens.length}END`;
      citationTokens.push({ citation, token });
      return token;
    },
  );
  let html = markdownToRichTextHtml(tokenizedMarkdown);

  citationTokens.forEach(({ citation, token }, tokenIndex) => {
    const citationIndex = findCitationIndex(citations, citation);
    const resolvedIndex = citationIndex >= 0 ? citationIndex : 0;
    const label = escapeHtmlAttribute(getInlineCitationAriaLabel(locale, citation, resolvedIndex));
    const displayId = escapeHtmlAttribute(getCitationDisplayId(citation, resolvedIndex));
    const buttonHtml = `<button aria-label="${label}" class="${styles.inlineCitationButton}" data-inline-citation-token-index="${tokenIndex}" type="button">${displayId}</button>`;

    html = html.split(token).join(buttonHtml);
  });

  return {
    citationTokens,
    html,
  };
}

type RenderedAnswerToken =
  | {
      type: "text";
      text: string;
    }
  | {
      type: "citation";
      displayId: string;
    }
  | {
      type: "image";
      displayId: string;
    };

function tokenizeRenderedAnswerLine(line: string) {
  const tokens: RenderedAnswerToken[] = [];
  const pattern = /\[\[image:(\d+)\]\]|\[\[(\d+)\]\]|\[(\d+)\]/gu;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(line)) !== null) {
    if (match.index > lastIndex) {
      tokens.push({
        type: "text",
        text: line.slice(lastIndex, match.index),
      });
    }

    if (match[1]) {
      tokens.push({
        type: "image",
        displayId: match[1],
      });
    } else {
      tokens.push({
        type: "citation",
        displayId: match[2] ?? match[3] ?? "",
      });
    }

    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < line.length) {
    tokens.push({
      type: "text",
      text: line.slice(lastIndex),
    });
  }

  return tokens;
}

function getModeLabel(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return formatUiMessage(locale, "workspaceChatModeRag");
    case "search":
      return formatUiMessage(locale, "workspaceChatModeSearch");
    case "chat":
    default:
      return formatUiMessage(locale, "workspaceChatModeChat");
  }
}

function getModeCode(mode: WorkspaceChatMode) {
  switch (mode) {
    case "rag":
      return "RAG";
    case "search":
      return "web_search";
    case "chat":
    default:
      return "chat";
  }
}

function getModeIndex(mode: WorkspaceChatMode) {
  return Math.max(CHAT_MODE_ORDER.indexOf(mode), 0);
}

function getAssistantMessageKey(messageId: number) {
  return `assistant-${messageId}`;
}

function normalizeStreamMessageId(messageId: number) {
  return messageId > 0 ? messageId : null;
}

function getMessageActions(locale: "zh-CN" | "en", role: PaneMessage["role"]) {
  if (role === "user") {
    return [
      formatUiMessage(locale, "workspaceChatActionCopy"),
      formatUiMessage(locale, "workspaceChatActionEdit"),
    ];
  }

  return [
    formatUiMessage(locale, "workspaceChatActionCopy"),
    formatUiMessage(locale, "workspaceChatActionAddToNote"),
    formatUiMessage(locale, "workspaceChatActionRegenerate"),
  ];
}

function isResearchMode(mode: WorkspaceChatMode) {
  return mode === "rag" || mode === "search";
}

function getProgressHeading(locale: "zh-CN" | "en", mode: WorkspaceChatMode) {
  if (locale === "zh-CN") {
    return mode === "rag" ? "知识库检索中" : "网络搜索中";
  }

  return mode === "rag" ? "Knowledge Retrieval" : "Web Search";
}

function getProgressToggleLabel(locale: "zh-CN" | "en", collapsed: boolean) {
  if (locale === "zh-CN") {
    return collapsed ? "展开过程" : "收起过程";
  }

  return collapsed ? "Expand progress" : "Collapse progress";
}

function getCompactStatusTitle(locale: "zh-CN" | "en") {
  return locale === "zh-CN" ? "正在思考" : "Thinking";
}

function getProgressCountLabel(locale: "zh-CN" | "en", key: string) {
  if (locale === "zh-CN") {
    switch (key) {
      case "queries":
        return "查询";
      case "results":
        return "结果";
      case "sources":
        return "来源";
      case "chunks":
        return "片段";
      case "documents":
        return "文档";
      default:
        return key;
    }
  }

  switch (key) {
    case "queries":
      return "queries";
    case "results":
      return "results";
    case "sources":
      return "sources";
    case "chunks":
      return "chunks";
    case "documents":
      return "documents";
    default:
      return key;
  }
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

type ResearchProgressCardProps = {
  activities: ProgressEntry[];
  collapsed: boolean;
  locale: "zh-CN" | "en";
  mode: WorkspaceChatMode;
  onToggleCollapsed: () => void;
};

function ResearchProgressCard({
  activities,
  collapsed,
  locale,
  mode,
  onToggleCollapsed,
}: ResearchProgressCardProps) {
  const bodyRef = useRef<HTMLDivElement | null>(null);
  const researchMode = isResearchMode(mode);
  const headerTitle = researchMode ? getProgressHeading(locale, mode) : getCompactStatusTitle(locale);
  const disclosureChevron = collapsed ? "▸" : "▾";

  useEffect(() => {
    if (!researchMode || collapsed) {
      return;
    }

    const body = bodyRef.current;

    if (!body) {
      return;
    }

    body.scrollTop = body.scrollHeight;
  }, [activities, collapsed]);

  return (
    <section
      className={[
        styles.progressCard,
        researchMode ? styles.progressCardResearch : styles.progressCardCompact,
      ]
        .filter(Boolean)
        .join(" ")}
      data-testid={researchMode ? "workspace-progress-card" : "workspace-status-hint"}
    >
      <div className={styles.progressHeader}>
        <span
          aria-hidden="true"
          className={[
            styles.progressIcon,
            mode === "rag"
              ? styles.progressIconRag
              : mode === "search"
                ? styles.progressIconSearch
                : styles.progressIconGeneral,
          ]
            .filter(Boolean)
            .join(" ")}
        >
          <span className={styles.progressIconCore} />
        </span>

        <div className={styles.progressHeaderMain}>
          <div className={styles.progressHeaderTitleRow}>
            <button
              aria-label={getProgressToggleLabel(locale, collapsed)}
              className={styles.progressDisclosure}
              onClick={onToggleCollapsed}
              type="button"
            >
              <span>{headerTitle}</span>
              <span className={styles.progressDisclosureChevron} aria-hidden="true">
                {disclosureChevron}
              </span>
            </button>
          </div>
        </div>
      </div>

      {!collapsed && activities.length > 0 ? (
        <div className={styles.progressBody} ref={bodyRef}>
          {activities.map((activity) => (
            <div className={styles.progressItem} key={activity.id}>
              <span aria-hidden="true" className={styles.progressItemDot} />
              <div className={styles.progressItemContent}>
                <div className={styles.progressItemHeader}>
                  <strong>{activity.title}</strong>
                  {activity.timestamp ? <span>{activity.timestamp}</span> : null}
                </div>
                {activity.detail ? <p className={styles.progressItemDetail}>{activity.detail}</p> : null}
                {Object.keys(activity.counts).length > 0 ? (
                  <div className={styles.progressMetaRow}>
                    {Object.entries(activity.counts).map(([key, value]) => (
                      <span className={styles.progressMetaPill} key={`${activity.id}-${key}`}>
                        {getProgressCountLabel(locale, key)} {value}
                      </span>
                    ))}
                  </div>
                ) : null}
                {activity.sourcesPreview.length > 0 ? (
                  <div className={styles.progressMetaRow}>
                    {activity.sourcesPreview.map((source) => (
                      <span className={styles.progressSourcePill} key={`${activity.id}-${source.id}`}>
                        {source.label}
                      </span>
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}

type ToolResultCardProps = {
  locale: "zh-CN" | "en";
  result: ToolResult;
};

/// Maps backend skill id to frontend render hint.
/// Keep in sync with `SkillComponent::render_hint()` in the Rust backend.
const TOOL_RENDER_HINTS: Record<string, string> = {
  calculator: "calculator",
  code_interpreter: "code",
  weather_query: "weather",
  web_search: "search",
};

function getToolRenderHint(toolName: string): string {
  return TOOL_RENDER_HINTS[toolName] ?? "json";
}

export function ToolResultCard({ locale, result }: ToolResultCardProps) {
  const [expanded, setExpanded] = useState(true);
  const data = result.data ?? {};
  const isError = result.status === "error";
  const isOk = result.status === "ok";
  const renderHint = getToolRenderHint(result.tool);

  const statusClass = isOk
    ? styles.toolResultStatusOk
    : isError
      ? styles.toolResultStatusError
      : styles.toolResultStatusOther;

  const statusLabel =
    result.status === "ok"
      ? "OK"
      : result.status === "error"
        ? locale === "zh-CN"
          ? "错误"
          : "Error"
        : result.status === "timeout"
          ? locale === "zh-CN"
            ? "超时"
            : "Timeout"
          : result.status === "not_found"
            ? locale === "zh-CN"
              ? "未找到"
              : "Not Found"
            : result.status === "not_implemented"
              ? locale === "zh-CN"
                ? "未实现"
                : "Not Implemented"
              : result.status;

  function renderBody() {
    if (renderHint === "code") {
      const stdout = typeof data.stdout === "string" ? data.stdout : "";
      const stderr = typeof data.stderr === "string" ? data.stderr : "";
      const execResult = data.result ?? "";
      const success = data.success === true;

      return (
        <div className={styles.toolResultBody}>
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "错误" : "Error"}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
          {execResult !== "" ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "返回值" : "Result"}
              </div>
              <pre>{typeof execResult === "string" ? execResult : JSON.stringify(execResult, null, 2)}</pre>
            </div>
          ) : null}
          {stdout ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>stdout</div>
              <pre>{stdout}</pre>
            </div>
          ) : null}
          {stderr ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>stderr</div>
              <pre style={{ color: "hsl(0 84% 60%)" }}>{stderr}</pre>
            </div>
          ) : null}
          {!success && data.exit_code !== undefined ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "退出码" : "Exit Code"}
              </div>
              <pre>{String(data.exit_code)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "calculator") {
      const expression = typeof data.expression === "string" ? data.expression : "";
      const calcResult = data.result !== undefined ? String(data.result) : "";

      return (
        <div className={styles.toolResultBody}>
          {expression ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "表达式" : "Expression"}
              </div>
              <pre>{expression}</pre>
            </div>
          ) : null}
          {calcResult ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "结果" : "Result"}
              </div>
              <pre>{calcResult}</pre>
            </div>
          ) : null}
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "错误" : "Error"}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "weather") {
      const location = typeof data.location === "string" ? data.location : "";
      const description = typeof data.description === "string" ? data.description : "";
      const temperature = data.temperature !== undefined ? String(data.temperature) : "";
      const feelsLike = data.feels_like !== undefined ? String(data.feels_like) : "";
      const humidity = data.humidity !== undefined ? String(data.humidity) : "";
      const windSpeed = data.wind_speed !== undefined ? String(data.wind_speed) : "";
      const units = typeof data.units === "string" ? data.units : "";

      return (
        <div className={styles.toolResultBody}>
          {location || description ? (
            <div style={{ fontWeight: 600, marginBottom: "0.4rem" }}>
              {location}
              {location && description ? " — " : ""}
              {description}
            </div>
          ) : null}
          <div className={styles.toolResultWeatherGrid}>
            {temperature ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "温度" : "Temperature"}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {temperature}
                  {units === "metric" ? "°C" : units === "imperial" ? "°F" : ""}
                </span>
              </div>
            ) : null}
            {feelsLike ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "体感" : "Feels Like"}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {feelsLike}
                  {units === "metric" ? "°C" : units === "imperial" ? "°F" : ""}
                </span>
              </div>
            ) : null}
            {humidity ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "湿度" : "Humidity"}
                </span>
                <span className={styles.toolResultWeatherValue}>{humidity}%</span>
              </div>
            ) : null}
            {windSpeed ? (
              <div className={styles.toolResultWeatherItem}>
                <span className={styles.toolResultWeatherLabel}>
                  {locale === "zh-CN" ? "风速" : "Wind Speed"}
                </span>
                <span className={styles.toolResultWeatherValue}>
                  {windSpeed}
                  {units === "metric" ? " m/s" : units === "imperial" ? " mph" : ""}
                </span>
              </div>
            ) : null}
          </div>
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "错误" : "Error"}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    if (renderHint === "search") {
      const results = Array.isArray(data.results) ? data.results : [];
      const answer = typeof data.synthesized_answer === "string" ? data.synthesized_answer : "";

      return (
        <div className={styles.toolResultBody}>
          {answer ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "摘要" : "Summary"}
              </div>
              <div style={{ lineHeight: 1.5 }}>{answer}</div>
            </div>
          ) : null}
          {results.length > 0 ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "搜索结果" : "Search Results"}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
                {results.map((r: any, i: number) => (
                  <div
                    key={i}
                    style={{
                      padding: "0.4rem 0.5rem",
                      borderRadius: "6px",
                      background: "hsl(var(--muted) / 0.15)",
                    }}
                  >
                    {r.url ? (
                      <a
                        href={r.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        style={{
                          fontWeight: 600,
                          fontSize: "0.85rem",
                          color: "hsl(217 91% 60%)",
                          textDecoration: "none",
                        }}
                      >
                        {typeof r.title === "string" ? r.title : r.url}
                      </a>
                    ) : (
                      <div style={{ fontWeight: 600, fontSize: "0.85rem" }}>
                        {typeof r.title === "string" ? r.title : ""}
                      </div>
                    )}
                    {typeof r.snippet === "string" && r.snippet ? (
                      <div style={{ fontSize: "0.78rem", color: "hsl(var(--muted-foreground))", marginTop: "0.15rem" }}>
                        {r.snippet}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            </div>
          ) : null}
          {data.error ? (
            <div className={styles.toolResultSection}>
              <div className={styles.toolResultSectionLabel}>
                {locale === "zh-CN" ? "错误" : "Error"}
              </div>
              <pre>{String(data.error)}</pre>
            </div>
          ) : null}
        </div>
      );
    }

    // Generic fallback: render data as JSON
    return (
      <div className={styles.toolResultBody}>
        <pre>{JSON.stringify(data, null, 2)}</pre>
      </div>
    );
  }

  const toolLabel =
    renderHint === "code"
      ? locale === "zh-CN"
        ? "代码执行"
        : "Code Execution"
      : renderHint === "calculator"
        ? locale === "zh-CN"
          ? "计算器"
          : "Calculator"
        : renderHint === "weather"
          ? locale === "zh-CN"
            ? "天气查询"
            : "Weather"
          : renderHint === "search"
            ? locale === "zh-CN"
              ? "网页搜索"
              : "Web Search"
            : result.tool;

  return (
    <div className={styles.toolResultCard}>
      <button
        className={styles.toolResultHeader}
        onClick={() => setExpanded((prev) => !prev)}
        type="button"
      >
        <span className={styles.toolResultTitle}>
          {toolLabel}
          <span className={[styles.toolResultStatus, statusClass].join(" ")}>{statusLabel}</span>
        </span>
        <span style={{ fontSize: "0.75rem", color: "hsl(var(--muted-foreground))" }}>
          {expanded ? "▾" : "▸"}
        </span>
      </button>
      {expanded ? renderBody() : null}
    </div>
  );
}

type ToolResultsPanelProps = {
  locale: "zh-CN" | "en";
  results: ToolResult[];
};

export function ToolResultsPanel({ locale, results }: ToolResultsPanelProps) {
  if (results.length === 0) {
    return null;
  }

  return (
    <div className={styles.toolResultsPanel}>
      {results.map((result, index) => (
        <ToolResultCard key={`${result.tool}-${index}`} locale={locale} result={result} />
      ))}
    </div>
  );
}

type AssistantAnswerContentProps = {
  locale: "zh-CN" | "en";
  message: PaneMessage;
  onOpenWebSources?: (request: WorkspaceWebSourcesRequest) => void;
  onSelectCitation: (citation: Citation, target: HTMLElement) => void;
};

function AssistantAnswerContent({
  locale,
  message,
  onOpenWebSources,
  onSelectCitation,
}: AssistantAnswerContentProps) {
  function handleCitationClick(citation: Citation, target: HTMLElement) {
    const webSource = message.mode === "search" ? citationToWebSource(citation) : null;

    if (webSource && onOpenWebSources) {
      onOpenWebSources({ sources: [webSource] });
      return;
    }

    onSelectCitation(citation, target);
  }

  function renderCitationButton(citation: Citation, key: string) {
    const citationIndex = findCitationIndex(message.citations, citation);
    const resolvedIndex = citationIndex >= 0 ? citationIndex : 0;
    const label = getCitationLabel(citation, resolvedIndex);
    const pageText = getCitationPageText(locale, citation.page);
    const preview = citation.preview?.trim() || citation.content?.trim() || "";
    const url = getCitationUrl(citation);
    let hoverTitle = pageText
      ? `${label} (${pageText})\n${preview}`
      : `${label}\n${preview}`;
    if (url) {
      hoverTitle += `\n${url}`;
    }

    return (
      <button
        aria-label={getInlineCitationAriaLabel(locale, citation, resolvedIndex)}
        className={styles.inlineCitationButton}
        data-testid="workspace-citation"
        key={key}
        onClick={(event) => {
          handleCitationClick(citation, event.currentTarget);
        }}
        title={hoverTitle.slice(0, 300)}
        type="button"
      >
        {getCitationDisplayId(citation, resolvedIndex)}
      </button>
    );
  }

  function renderImageCard(citation: Citation, key: string) {
    const citationIndex = findCitationIndex(message.citations, citation);
    const resolvedIndex = citationIndex >= 0 ? citationIndex : 0;
    const imageUrl = citation.image_url?.trim();
    const caption = citation.caption?.trim() || getCitationLabel(citation, resolvedIndex);

    return (
      <button
        aria-label={getInlineCitationAriaLabel(locale, citation, resolvedIndex)}
        className={styles.answerImageCard}
        key={key}
        onClick={(event) => {
          onSelectCitation(citation, event.currentTarget);
        }}
        type="button"
      >
        {imageUrl ? (
          <img
            alt={caption}
            className={styles.answerImage}
            src={imageUrl}
          />
        ) : (
          <span className={styles.answerImageFallback}>
            {locale === "zh-CN" ? "查看图片引用" : "Open cited image"}
          </span>
        )}
        <span className={styles.answerImageMeta}>
          <span className={styles.answerImageBadge}>
            {getCitationDisplayId(citation, resolvedIndex)}
          </span>
          <span className={styles.answerImageCaption}>{caption}</span>
        </span>
      </button>
    );
  }

  if (message.answerBlocks.length > 0) {
    if (hasOnlyTextAnswerBlocks(message.answerBlocks)) {
      const mergedText = getAnswerBlockText(message.answerBlocks);
      const richMarkdown = markdownToRichTextHtmlWithCitationButtons(
        mergedText,
        message.citations,
        locale,
      );
      // Fallback: if the LLM did not insert [[N]] markers into the text but
      // citations were returned separately, render them as a trailing group.
      const trailingCitations =
        richMarkdown.citationTokens.length === 0 && message.citations.length > 0
          ? dedupeCitations(message.citations)
          : [];

      return (
        <>
          <div
            className={styles.markdownContent}
            onClick={(event) => {
              const target = event.target as HTMLElement;
              const button = target.closest<HTMLButtonElement>(
                "button[data-inline-citation-token-index]",
              );

              if (!button) {
                return;
              }

              const tokenIndex = Number.parseInt(
                button.dataset.inlineCitationTokenIndex ?? "",
                10,
              );
              const citation = richMarkdown.citationTokens[tokenIndex]?.citation;

              if (citation) {
                handleCitationClick(citation, button);
              }
            }}
            dangerouslySetInnerHTML={{
              __html: richMarkdown.html,
            }}
          />
          {trailingCitations.length > 0 ? (
            <div className={styles.inlineCitationGroup} style={{ marginTop: "0.5rem" }}>
              {trailingCitations.map((citation, idx) =>
                renderCitationButton(citation, `trailing-${idx}`),
              )}
            </div>
          ) : null}
          <ToolResultsPanel locale={locale} results={message.toolResults} />
        </>
      );
    }

    return (
      <>
        <div className={styles.answerBlockStack}>
          {message.answerBlocks.map((block, blockIndex) => {
            if (block.type === "image") {
              const citation = findCitationByChunkId(message.citations, block.chunk_id);

              if (!citation) {
                return null;
              }

              return renderImageCard(citation, `image-${blockIndex}`);
            }

            const blockHtml = markdownToInlineHtml(block.text);
            const blockCitations = dedupeCitations(
              block.citations.map((chunkId) => findCitationByChunkId(message.citations, chunkId)),
            );

            if (!blockHtml && blockCitations.length === 0) {
              return null;
            }

            return (
              <p className={styles.answerTextBlock} key={`text-${blockIndex}`}>
                {blockHtml ? (
                  <span
                    dangerouslySetInnerHTML={{
                      __html: blockHtml,
                    }}
                  />
                ) : null}
                {blockCitations.length > 0 ? (
                  <span className={styles.inlineCitationGroup}>
                    {blockCitations.map((citation, citationIndex) =>
                      renderCitationButton(citation, `block-${blockIndex}-${citationIndex}`),
                    )}
                  </span>
                ) : null}
              </p>
            );
          })}
        </div>
        <ToolResultsPanel locale={locale} results={message.toolResults} />
      </>
    );
  }

  if (hasRenderedCitationMarkup(message.content)) {
    let inlineCitationsRendered = 0;

    const renderedLines = message.content.split("\n").map((line, lineIndex) => {
      const trimmedLine = line.trim();

      if (!trimmedLine) {
        return <div aria-hidden="true" className={styles.answerSpacer} key={`spacer-${lineIndex}`} />;
      }

      const tokens = tokenizeRenderedAnswerLine(line);

      if (tokens.length === 1 && tokens[0]?.type === "image") {
        const citation = findCitationByDisplayId(message.citations, tokens[0].displayId);

        if (!citation) {
          return null;
        }

        inlineCitationsRendered += 1;
        return renderImageCard(citation, `rendered-image-${lineIndex}`);
      }

      return (
        <p className={styles.answerTextBlock} key={`line-${lineIndex}`}>
          {tokens.map((token, tokenIndex) => {
            if (token.type === "text") {
              if (!token.text) {
                return null;
              }

              return (
                <span
                  dangerouslySetInnerHTML={{
                    __html: markdownToInlineHtml(token.text),
                  }}
                  key={`text-${lineIndex}-${tokenIndex}`}
                />
              );
            }

            if (token.type === "citation") {
              const citation = findCitationByDisplayId(message.citations, token.displayId);

              if (!citation) {
                return (
                  <span className={styles.inlineCitationFallback} key={`fallback-${lineIndex}-${tokenIndex}`}>
                    [{token.displayId}]
                  </span>
                );
              }

              inlineCitationsRendered += 1;
              return renderCitationButton(citation, `inline-${lineIndex}-${tokenIndex}`);
            }

            const citation = findCitationByDisplayId(message.citations, token.displayId);

            if (!citation) {
              return (
                <span className={styles.inlineCitationFallback} key={`image-fallback-${lineIndex}-${tokenIndex}`}>
                  [image {token.displayId}]
                </span>
              );
            }

            inlineCitationsRendered += 1;
            return renderCitationButton(citation, `image-inline-${lineIndex}-${tokenIndex}`);
          })}
        </p>
      );
    });

    // Fallback: if inline citation markup was detected but no matching citations
    // were found (e.g., LLM hallucinated wrong indices), render trailing buttons.
    const trailingCitationsMarkup =
      inlineCitationsRendered === 0 && message.citations.length > 0
        ? dedupeCitations(message.citations)
        : [];

    return (
      <>
        <div className={styles.answerBlockStack}>{renderedLines}</div>
        {trailingCitationsMarkup.length > 0 ? (
          <div className={styles.inlineCitationGroup} style={{ marginTop: "0.5rem" }}>
            {trailingCitationsMarkup.map((citation, idx) =>
              renderCitationButton(citation, `trailing-${idx}`),
            )}
          </div>
        ) : null}
        <ToolResultsPanel locale={locale} results={message.toolResults} />
      </>
    );
  }

  // HTML format output: if the raw answer looks like a complete HTML document,
  // render it directly instead of running it through the markdown parser (which
  // would escape the tags and wrap it in <pre><code>).
  const rawContent = message.content || (message.pending ? "..." : "");
  const looksLikeHtml = /^\s*<(!doctype\s+html|html)/iu.test(rawContent);

  // Search and other plain-text modes may return citations without answerBlocks.
  // Render them as a trailing group when no inline markup was detected.
  const trailingCitationsFallback =
    message.citations.length > 0 ? dedupeCitations(message.citations) : [];

  if (looksLikeHtml) {
    return (
      <>
        <div
          className={styles.markdownContent}
          dangerouslySetInnerHTML={{ __html: rawContent }}
        />
        {trailingCitationsFallback.length > 0 ? (
          <div className={styles.inlineCitationGroup} style={{ marginTop: "0.5rem" }}>
            {trailingCitationsFallback.map((citation, idx) =>
              renderCitationButton(citation, `trailing-${idx}`),
            )}
          </div>
        ) : null}
        <ToolResultsPanel locale={locale} results={message.toolResults} />
      </>
    );
  }

  return (
    <>
      <div
        className={styles.markdownContent}
        dangerouslySetInnerHTML={{
          __html: markdownToRichTextHtml(rawContent),
        }}
      />
      {trailingCitationsFallback.length > 0 ? (
        <div className={styles.inlineCitationGroup} style={{ marginTop: "0.5rem" }}>
          {trailingCitationsFallback.map((citation, idx) =>
            renderCitationButton(citation, `trailing-${idx}`),
          )}
        </div>
      ) : null}
      <ToolResultsPanel locale={locale} results={message.toolResults} />
    </>
  );
}

export function WorkspaceChatPane({
  workspaceId,
  sessionId,
  selectedSourceIds,
  onSessionActivity,
  onSessionChange,
  onFocusSource: _onFocusSource,
  onSelectCitation,
  onOpenWebSources,
}: WorkspaceChatPaneProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const workspaceUi = useWorkspaceUi(workspaceId);
  const { setChatMode } = workspaceUi;
  const [messages, setMessages] = useState<PaneMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(sessionId);
  const [streamingMessageId, setStreamingMessageId] = useState<string | null>(null);
  const [progressMode, setProgressMode] = useState<WorkspaceChatMode | null>(null);
  const [progressActivities, setProgressActivities] = useState<ProgressEntry[]>([]);
  const [progressCollapsed, setProgressCollapsed] = useState(true);
  const [showModeMenu, setShowModeMenu] = useState(false);
  const [modeMenuActiveIndex, setModeMenuActiveIndex] = useState(0);
  const [composerClearance, setComposerClearance] = useState<number | null>(null);
  const [composerTextareaHeight, setComposerTextareaHeight] = useState<number | null>(null);
  const [isComposerResizing, setIsComposerResizing] = useState(false);
  const streamingSessionIdRef = useRef<string | null>(null);
  const streamingMessageIdRef = useRef<string | null>(null);
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const composerCardRef = useRef<HTMLDivElement | null>(null);
  const composerResizeCleanupRef = useRef<(() => void) | null>(null);
  const modeMenuRef = useRef<HTMLDivElement | null>(null);
  const streamTypewriterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const streamTypewriterQueueRef = useRef("");
  const streamDisplayedTextRef = useRef("");
  const streamReceivedTokenRef = useRef(false);
  const streamReduceMotionRef = useRef(false);
  const stopControllerRef = useRef<AbortController | null>(null);
  const pendingDoneEventRef = useRef<PendingDoneEvent | null>(null);
  const progressModeRef = useRef<WorkspaceChatMode | null>(null);

  useEffect(() => {
    let cancelled = false;

    resetStreamingTypewriter();
    setActiveSessionId(sessionId);
    setMessages([]);
    setError("");
    progressModeRef.current = null;
    setProgressMode(null);
    setProgressActivities([]);
    setProgressCollapsed(true);
    streamingSessionIdRef.current = sessionId;
    streamingMessageIdRef.current = null;

    if (!sessionId || !auth.token) {
      return () => {
        cancelled = true;
      };
    }

    const token = auth.token;
    const transcriptSessionId = sessionId;

    void (async () => {
      try {
        const response = await listWorkspaceSessionMessages(token, transcriptSessionId);

        if (cancelled) {
          return;
        }

        setMessages(response.messages.map(mapTranscriptMessage));
      } catch {
        if (!cancelled) {
          setError(formatUiMessage(locale, "workspaceChatLoadError"));
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [auth.token, locale, sessionId, workspaceId]);

  useEffect(() => {
    const textarea = textareaRef.current;

    if (!textarea) {
      return;
    }

    textarea.style.height = "0px";
    const contentHeight = Math.max(textarea.scrollHeight, MIN_COMPOSER_TEXTAREA_HEIGHT);
    const nextTextareaHeight =
      composerTextareaHeight === null
        ? Math.min(contentHeight, AUTO_COMPOSER_TEXTAREA_MAX_HEIGHT)
        : Math.min(
            Math.max(contentHeight, composerTextareaHeight),
            MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT,
          );
    textarea.style.height = `${nextTextareaHeight}px`;

    const composerCard = composerCardRef.current;

    if (!composerCard) {
      return;
    }

    const nextComposerClearance = Math.ceil(composerCard.getBoundingClientRect().height);

    if (nextComposerClearance > 0) {
      setComposerClearance((current) => (current === nextComposerClearance ? current : nextComposerClearance));
    }
  }, [composerTextareaHeight, draft]);

  useEffect(() => {
    const transcript = transcriptRef.current;

    if (!transcript) {
      return;
    }

    transcript.scrollTop = transcript.scrollHeight;
  }, [messages, isStreaming]);

  useEffect(() => {
    if (!showModeMenu) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      if (!modeMenuRef.current?.contains(event.target as Node)) {
        setShowModeMenu(false);
      }
    }

    window.addEventListener("mousedown", handlePointerDown);

    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
    };
  }, [showModeMenu]);

  useEffect(() => {
    if (!composerCardRef.current) {
      return;
    }

    function updateComposerClearance() {
      const composerCard = composerCardRef.current;

      if (!composerCard) {
        return;
      }

      const nextComposerClearance = Math.ceil(composerCard.getBoundingClientRect().height);

      if (nextComposerClearance <= 0) {
        return;
      }

      setComposerClearance((current) => (current === nextComposerClearance ? current : nextComposerClearance));
    }

    updateComposerClearance();
    window.addEventListener("resize", updateComposerClearance);

    if (typeof ResizeObserver === "undefined") {
      return () => {
        window.removeEventListener("resize", updateComposerClearance);
      };
    }

    const observer = new ResizeObserver(() => {
      updateComposerClearance();
    });

    observer.observe(composerCardRef.current);

    return () => {
      observer.disconnect();
      window.removeEventListener("resize", updateComposerClearance);
    };
  }, []);

  useEffect(() => {
    return () => {
      composerResizeCleanupRef.current?.();
      resetStreamingTypewriter();
    };
  }, []);

  const transcript = useMemo(() => messages, [messages]);
  const effectiveChatMode = resolveWorkspaceChatMode(workspaceUi, selectedSourceIds.length > 0);
  const activeModeLabel = getModeLabel(locale, effectiveChatMode);
  const activeModeCode = getModeCode(effectiveChatMode);
  const shellStyle =
    composerClearance !== null
      ? ({
          "--workspace-chat-bottom-clearance": `${composerClearance}px`,
        } as CSSProperties)
      : undefined;

  function handleComposerResizeStart(event: ReactMouseEvent<HTMLButtonElement>) {
    if (event.button !== 0) {
      return;
    }

    const textarea = textareaRef.current;

    if (!textarea) {
      return;
    }

    event.preventDefault();
    composerResizeCleanupRef.current?.();

    const startingHeight = Math.max(
      Number.parseFloat(textarea.style.height) || 0,
      textarea.clientHeight,
      MIN_COMPOSER_TEXTAREA_HEIGHT,
    );
    const startY = event.clientY;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;

    setIsComposerResizing(true);
    document.body.style.cursor = "ns-resize";
    document.body.style.userSelect = "none";

    function handleMouseMove(moveEvent: MouseEvent) {
      const nextHeight = Math.min(
        Math.max(startingHeight + (startY - moveEvent.clientY), MIN_COMPOSER_TEXTAREA_HEIGHT),
        MANUAL_COMPOSER_TEXTAREA_MAX_HEIGHT,
      );

      setComposerTextareaHeight(nextHeight);
    }

    function cleanup() {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      setIsComposerResizing(false);
      composerResizeCleanupRef.current = null;
    }

    function handleMouseUp() {
      cleanup();
    }

    composerResizeCleanupRef.current = cleanup;
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function showProgressCard(mode: WorkspaceChatMode) {
    progressModeRef.current = mode;
    setProgressMode(mode);
    setProgressActivities(isResearchMode(mode) ? [getInitialProgressEntry(locale, mode)] : []);
    setProgressCollapsed(true);
  }

  function hideProgressCard() {
    progressModeRef.current = null;
    setProgressMode(null);
    setProgressActivities([]);
    setProgressCollapsed(true);
  }

  function openModeMenu() {
    setModeMenuActiveIndex(getModeIndex(effectiveChatMode));
    setShowModeMenu(true);
  }

  function applyModeSelection(mode: WorkspaceChatMode) {
    setChatMode(mode);
    setModeMenuActiveIndex(getModeIndex(mode));
    setShowModeMenu(false);
    textareaRef.current?.focus();
  }

  function handleCopyMessage(content: string) {
    if (typeof navigator === "undefined" || !navigator.clipboard) {
      return;
    }

    void navigator.clipboard.writeText(content);
  }

  function getCopyableMessageContent(message: PaneMessage) {
    if (message.role === "assistant") {
      const answerText = getAnswerBlockText(message.answerBlocks).trim();

      if (answerText) {
        return answerText;
      }
    }

    return message.content;
  }

  function handleEditMessage(content: string) {
    setDraft(content);
    textareaRef.current?.focus();
  }

  async function handleSubmitFeedback(message: PaneMessage, rating: "up" | "down") {
    if (!auth.token || !message.sessionId || message.messageId === null) {
      return;
    }

    setMessages((prev) =>
      prev.map((m) =>
        m.id === message.id ? { ...m, feedbackRating: rating } : m,
      ),
    );

    try {
      await submitWorkspaceMessageFeedback(auth.token, {
        session_id: message.sessionId,
        message_id: message.messageId,
        rating,
      });
    } catch {
      // Silently fail — feedback is best-effort
    }
  }

  function handleCitationSelect(message: PaneMessage, citation: Citation, target?: HTMLElement | null) {
    if (message.sessionId && message.messageId !== null) {
      onSelectCitation?.({
        session_id: message.sessionId,
        message_id: message.messageId,
        citation,
        anchorRect: target ? getCitationAnchorRect(target) : null,
      });
    }
  }

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

  function updateStreamingAssistant(
    updater: (current: PaneMessage | null) => PaneMessage,
    targetId?: string | null,
    fallbackId?: string | null,
  ) {
    const candidateIds = [targetId ?? streamingMessageIdRef.current, fallbackId].filter(
      (value): value is string => Boolean(value),
    );

    if (candidateIds.length === 0) {
      return;
    }

    setMessages((current) => {
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
        mode: eventMode ?? current?.mode ?? effectiveChatMode,
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
      mode: current?.mode ?? effectiveChatMode,
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
        mode: normalizeMessageMode(event.payload.agent_type) ?? current?.mode ?? effectiveChatMode,
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
    onSessionChange?.(event.session_id);
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
    streamTypewriterQueueRef.current = streamTypewriterQueueRef.current.slice(
      STREAM_TYPEWRITER_CHARS_PER_TICK,
    );
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

    streamTypewriterTimerRef.current = setTimeout(
      flushStreamingTypewriterQueue,
      STREAM_TYPEWRITER_INTERVAL_MS,
    );
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

    setMessages((current) =>
      current.map((message) =>
        message.id === pendingMessageId ? { ...message, pending: false } : message,
      ),
    );
  }

  function beginAnswerStreaming(event: Extract<WorkspaceChatStreamEvent, { kind: "answer_start" }>) {
    ensureStreamingAssistant(event);
  }

  function handleStreamEvent(event: WorkspaceChatStreamEvent) {
    switch (event.kind) {
      case "start":
        if (event.session_id) {
          streamingSessionIdRef.current = event.session_id;
          setActiveSessionId(event.session_id);
          onSessionChange?.(event.session_id);
        }
        break;
      case "activity":
        setProgressActivities((current) => [
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
        break;
      case "answer_start":
        if (normalizeMessageMode(event.agent_type) !== "chat") {
          beginAnswerStreaming(event);
        }
        break;
      case "token": {
        const activeProgressMode = progressModeRef.current;
        if (!activeProgressMode || !isResearchMode(activeProgressMode)) {
          hideProgressCard();
        }
        ensureStreamingAssistant(event);
        streamReceivedTokenRef.current = true;
        enqueueStreamingText(event.content);
        break;
      }
      case "reasoning_summary_delta":
        setProgressActivities((current) => [
          ...current,
          {
            id: `reasoning-${current.length}-${Date.now()}`,
            phase: "reasoning",
            title: locale === "zh-CN" ? "正在整理思路" : "Reasoning summary",
            detail: event.content,
            counts: {},
            sourcesPreview: [],
            timestamp: null,
          },
        ]);
        break;
      case "citations":
        ensureStreamingAssistant(event);
        break;
      case "done": {
        hideProgressCard();
        handleDoneWithTypewriter(event);
        break;
      }
      case "error":
        hideProgressCard();
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
  }

  async function handleSubmit(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();

    const query = draft.trim();

    if (!query || isStreaming || !auth.token) {
      return;
    }

    const nextAssistantId = `assistant-${Date.now()}`;
    const requestSessionId = activeSessionId ?? sessionId;
    const authToken = auth.token;

    setError("");
    setIsStreaming(true);
    setStreamingMessageId(nextAssistantId);
    streamingMessageIdRef.current = nextAssistantId;
    resetStreamingTypewriter();
    setDraft("");
    setShowModeMenu(false);
    onSessionActivity?.();

    setMessages((current) => [
      ...current,
      {
        id: `user-${Date.now()}`,
        role: "user",
        mode: null,
        content: query,
        answerBlocks: [],
        citations: [],
        degradeTrace: [],
        guarded: false,
        messageId: null,
        sessionId: requestSessionId,
        toolResults: [],
      },
    ]);
    showProgressCard(effectiveChatMode);

    const controller = new AbortController();
    stopControllerRef.current = controller;

    void (async () => {
      try {
        await streamWorkspaceChat(
          authToken,
          {
            query,
            notebook_id: workspaceId,
            session_id: requestSessionId,
            agent_type: effectiveChatMode,
            doc_scope: selectedSourceIds,
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
        hideProgressCard();
        resetStreamingTypewriter();
        clearPendingStreamingAssistant();
        setError(submitError instanceof Error ? submitError.message : formatUiMessage(locale, "workspaceStreamError"));
        setIsStreaming(false);
        setStreamingMessageId(null);
        streamingMessageIdRef.current = null;
      } finally {
        if (stopControllerRef.current === controller) {
          stopControllerRef.current = null;
        }
      }
    })();
  }

function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (showModeMenu) {
      if (event.key === "Escape") {
        event.preventDefault();
        setShowModeMenu(false);
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setModeMenuActiveIndex((current) => (current + 1) % CHAT_MODE_ORDER.length);
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        setModeMenuActiveIndex((current) => (current - 1 + CHAT_MODE_ORDER.length) % CHAT_MODE_ORDER.length);
        return;
      }

      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        applyModeSelection(CHAT_MODE_ORDER[modeMenuActiveIndex] ?? effectiveChatMode);
        return;
      }
    }

    if (
      event.key === "/" &&
      !event.shiftKey &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      draft.trim().length === 0
    ) {
      event.preventDefault();
      openModeMenu();
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void handleSubmit();
    }
  }

  return (
    <section className={styles.shell} style={shellStyle} aria-label={formatUiMessage(locale, "workspaceChatRegionLabel")}>
      <header className={styles.header}>
        <div className={styles.titleBlock}>
          <h2 className={styles.title}>{activeModeLabel}</h2>
        </div>
        <span className={styles.modeChip}>{activeModeCode}</span>
      </header>

      <div className={styles.transcript} aria-label={formatUiMessage(locale, "workspaceTranscriptLabel")} ref={transcriptRef}>
        <div className={styles.transcriptInner}>
          {error ? <p className={styles.error}>{error}</p> : null}

          {transcript.length > 0 ? (
            transcript.map((message) => (
              <article
                className={[
                  styles.message,
                  message.role === "assistant" ? styles.messageAssistant : styles.messageUser,
                ]
                  .filter(Boolean)
                  .join(" ")}
                data-testid="workspace-message"
                data-role={message.role}
                key={message.id}
              >
                <div
                  className={[
                    styles.messageContent,
                    message.role === "assistant" ? styles.messageContentAssistant : styles.messageContentUser,
                  ]
                    .filter(Boolean)
                    .join(" ")}
                >
                  {message.role === "assistant" ? (
                    <div
                      className={[
                        styles.modeBubbleTag,
                        message.mode === "rag"
                          ? styles.modeBubbleTagRag
                          : message.mode === "search"
                            ? styles.modeBubbleTagSearch
                            : styles.modeBubbleTagGeneral,
                      ]
                        .filter(Boolean)
                        .join(" ")}
                    >
                      <span>{getModeLabel(locale, message.mode ?? "chat")}</span>
                      <span className={styles.modeBubbleTagCode}>{getModeCode(message.mode ?? "chat")}</span>
                    </div>
                  ) : null}

                  <div
                    className={[
                      styles.bubble,
                      message.role === "assistant"
                        ? [
                            styles.bubbleAssistant,
                            message.mode === "rag"
                              ? styles.bubbleAssistantRag
                              : message.mode === "search"
                                ? styles.bubbleAssistantSearch
                                : styles.bubbleAssistantGeneral,
                          ].join(" ")
                        : styles.bubbleUser,
                      message.pending ? styles.bubblePending : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    data-testid="workspace-answer-bubble"
                  >
                    {message.role === "assistant" ? (
                      <AssistantAnswerContent
                        locale={locale}
                        message={message}
                        onOpenWebSources={onOpenWebSources}
                        onSelectCitation={(citation, target) => {
                          handleCitationSelect(message, citation, target);
                        }}
                      />
                    ) : (
                      message.content || (message.pending ? "..." : "")
                    )}

                    {message.role === "assistant" &&
                    message.mode === "search" &&
                    !message.pending
                      ? (() => {
                          const webSources = collectWebSources(message.citations);
                          if (webSources.length === 0) {
                            return null;
                          }

                          return (
                            <button
                              className={styles.webSourceButton}
                              onClick={() =>
                                onOpenWebSources?.({ sources: webSources })
                              }
                              type="button"
                            >
                              {locale === "zh-CN"
                                ? `${webSources.length} 个来源`
                                : `${webSources.length} source${webSources.length > 1 ? "s" : ""}`}
                            </button>
                          );
                        })()
                      : null}
                  </div>

                  <div className={styles.messageActions}>
                    {getMessageActions(locale, message.role).map((action) => (
                      <button
                        className={styles.messageActionButton}
                        key={`${message.id}-${action}`}
                        type="button"
                        onClick={() => {
                          if (action === formatUiMessage(locale, "workspaceChatActionCopy")) {
                            handleCopyMessage(getCopyableMessageContent(message));
                          }

                          if (message.role === "user" && action === formatUiMessage(locale, "workspaceChatActionEdit")) {
                            handleEditMessage(message.content);
                          }
                        }}
                      >
                        {action}
                      </button>
                    ))}
                    {message.role === "assistant" && !message.pending ? (
                      <>
                        <button
                          aria-label={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                          className={[
                            styles.messageActionButton,
                            message.feedbackRating === "up" ? styles.messageActionButtonActive : "",
                          ].filter(Boolean).join(" ")}
                          disabled={message.feedbackRating === "up"}
                          type="button"
                          onClick={() => {
                            void handleSubmitFeedback(message, "up");
                          }}
                          title={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                        >
                          {message.feedbackRating === "up" ? "👍" : "🤙"}
                        </button>
                        <button
                          aria-label={formatUiMessage(locale, "workspaceChatActionThumbDown")}
                          className={[
                            styles.messageActionButton,
                            message.feedbackRating === "down" ? styles.messageActionButtonActive : "",
                          ].filter(Boolean).join(" ")}
                          disabled={message.feedbackRating === "down"}
                          type="button"
                          onClick={() => {
                            void handleSubmitFeedback(message, "down");
                          }}
                          title={formatUiMessage(locale, "workspaceChatActionThumbDown")}
                        >
                          {message.feedbackRating === "down" ? "👎" : "🫣"}
                        </button>
                      </>
                    ) : null}
                  </div>

                  {message.role === "assistant" && (message.guarded || message.degradeTrace.length > 0) ? (
                    <div className={styles.messageNotice}>
                      {message.guarded ? (
                        <div className={styles.messageNoticeTitle}>{formatUiMessage(locale, "workspaceGuardIntervened")}</div>
                      ) : null}
                      {message.degradeTrace.length > 0 ? (
                        <div className={styles.messageNoticeBody}>
                          {formatUiMessage(locale, "workspaceDegradeReasons", {
                            reasons: message.degradeTrace.map((entry) => entry.reason).join(" / "),
                          })}
                        </div>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              </article>
            ))
          ) : null}

          {progressMode ? (
            <ResearchProgressCard
              activities={progressActivities}
              collapsed={progressCollapsed}
              locale={locale}
              mode={progressMode}
              onToggleCollapsed={() => {
                setProgressCollapsed((current) => !current);
              }}
            />
          ) : null}
        </div>
      </div>

      <div className={styles.composerCard} ref={composerCardRef}>
        <button
          aria-label={locale === "zh-CN" ? "调整输入框高度" : "Resize composer"}
          className={`${styles.composerResizeHandle}${isComposerResizing ? ` ${styles.composerResizeHandleActive}` : ""}`}
          onMouseDown={handleComposerResizeStart}
          type="button"
        >
          <span className={styles.composerResizeGrip} aria-hidden="true" />
        </button>

        <form className={styles.composerForm} onSubmit={handleSubmit}>
          <label className={styles.srOnly} htmlFor={`workspace-chat-composer-${workspaceId}`}>
            {formatUiMessage(locale, "workspaceChatComposerLabel")}
          </label>

          <div className={styles.tagRow} ref={modeMenuRef}>
            <button
              aria-expanded={showModeMenu}
              aria-label={formatUiMessage(locale, "workspaceChatModeLabel")}
              className={`${styles.modeTag}${showModeMenu ? ` ${styles.modeTagOpen}` : ""}`}
              data-testid="workspace-chat-mode-button"
              onClick={() => {
                if (showModeMenu) {
                  setShowModeMenu(false);
                  return;
                }

                openModeMenu();
              }}
              type="button"
            >
              <span>{activeModeLabel}</span>
              <svg aria-hidden="true" className={styles.modeTagChevron} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path d="m7 14 5-5 5 5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.8" />
              </svg>
            </button>

            {showModeMenu ? (
              <div className={styles.modeMenu}>
                {CHAT_MODE_ORDER.map((mode, index) => (
                  <button
                    className={`${styles.modeMenuItem}${index === modeMenuActiveIndex ? ` ${styles.modeMenuItemActive}` : ""}`}
                    data-testid={`workspace-chat-mode-${mode}`}
                    key={mode}
                    onClick={() => applyModeSelection(mode)}
                    type="button"
                  >
                    <span className={styles.modeMenuItemLabel}>{getModeLabel(locale, mode)}</span>
                    <span className={styles.modeMenuItemCode}>{getModeCode(mode)}</span>
                  </button>
                ))}
              </div>
            ) : null}
          </div>

          <textarea
            className={styles.textarea}
            data-testid="workspace-chat-composer"
            id={`workspace-chat-composer-${workspaceId}`}
            onChange={(event) => {
              const nextDraft = event.target.value;
              setDraft(nextDraft);

              if (showModeMenu && nextDraft.trim().length > 0) {
                setShowModeMenu(false);
              }
            }}
            onKeyDown={handleKeyDown}
            placeholder={formatUiMessage(locale, "workspaceChatComposerPlaceholder")}
            ref={textareaRef}
            rows={1}
            value={draft}
          />

          <div className={styles.composerToolbar}>
            <div className={styles.toolbarLeft}>
              <p className={styles.hint}>{formatUiMessage(locale, "workspaceChatComposerHint")}</p>
            </div>

            <button
              aria-label={isStreaming ? formatUiMessage(locale, "workspaceSending") : formatUiMessage(locale, "workspaceSend")}
              className={styles.sendButton}
              data-testid="workspace-chat-send"
              disabled={isStreaming || draft.trim().length === 0}
              type="submit"
            >
              <svg aria-hidden="true" className={styles.sendIcon} fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path d="M12 18V6" strokeLinecap="round" strokeWidth="2" />
                <path d="m7.5 10.5 4.5-4.5 4.5 4.5" strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" />
              </svg>
              <span className={styles.srOnly}>
                {isStreaming ? formatUiMessage(locale, "workspaceSending") : formatUiMessage(locale, "workspaceSend")}
              </span>
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}
