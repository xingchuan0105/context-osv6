"use client";

import { useEffect, useRef, useState } from "react";
import { formatUiMessage } from "../../lib/i18n/messages";
import type {
  WorkspaceCitationAnchor,
  WorkspaceCitationRequest,
  WorkspaceWebSourcesRequest,
  WebSource,
} from "../../lib/workspace/model";
import type { WorkspaceChatMode } from "../../lib/workspace/ui-store";
import {
  type AnswerBlock,
  type Citation,
  type DegradeTraceItem,
  type ProgressSourcePreview,
  type ToolResult,
} from "../../lib/workspace/stream";
import { markdownToInlineHtml, markdownToRichTextHtml } from "./workspace-note-rich-text";
import styles from "./workspace-chat.module.css";
import type { ChatMessage, ProgressEntry } from "../../hooks/use-chat-session";

// =============================================================================
// Helpers
// =============================================================================

function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
}

function getCopyableMessageContent(message: ChatMessage) {
  if (message.role === "assistant") {
    const answerText = getAnswerBlockText(message.answerBlocks).trim();
    if (answerText) {
      return answerText;
    }
  }
  return message.content;
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

function getMessageActions(locale: "zh-CN" | "en", role: ChatMessage["role"]) {
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

function getInlineCitationAriaLabel(locale: "zh-CN" | "en", citation: Citation, index: number) {
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
  return citations.find((citation) => citation.chunk_id?.trim() === normalizedChunkId) ?? null;
}

function findCitationByDisplayId(citations: Citation[], displayId: string) {
  return (
    citations.find((citation, index) => getCitationDisplayId(citation, index) === displayId) ?? null
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

  return { citationTokens, html };
}

type RenderedAnswerToken =
  | { type: "text"; text: string }
  | { type: "citation"; displayId: string }
  | { type: "image"; displayId: string };

function tokenizeRenderedAnswerLine(line: string) {
  const tokens: RenderedAnswerToken[] = [];
  const pattern = /\[\[image:(\d+)\]\]|\[\[(\d+)\]\]|\[(\d+)\]/gu;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(line)) !== null) {
    if (match.index > lastIndex) {
      tokens.push({ type: "text", text: line.slice(lastIndex, match.index) });
    }
    if (match[1]) {
      tokens.push({ type: "image", displayId: match[1] });
    } else {
      tokens.push({ type: "citation", displayId: match[2] ?? match[3] ?? "" });
    }
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < line.length) {
    tokens.push({ type: "text", text: line.slice(lastIndex) });
  }

  return tokens;
}

// =============================================================================
// Sub-components
// =============================================================================

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
  }, [activities, collapsed, researchMode]);

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
                {activity.detail ? (
                  <p className={styles.progressItemDetail}>{activity.detail}</p>
                ) : null}
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
                      <div
                        style={{
                          fontSize: "0.78rem",
                          color: "hsl(var(--muted-foreground))",
                          marginTop: "0.15rem",
                        }}
                      >
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
  message: ChatMessage;
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
    let hoverTitle = pageText ? `${label} (${pageText})\n${preview}` : `${label}\n${preview}`;
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
          <img alt={caption} className={styles.answerImage} src={imageUrl} />
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
      const richMarkdown = markdownToRichTextHtmlWithCitationButtons(mergedText, message.citations, locale);
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
              const tokenIndex = Number.parseInt(button.dataset.inlineCitationTokenIndex ?? "", 10);
              const citation = richMarkdown.citationTokens[tokenIndex]?.citation;
              if (citation) {
                handleCitationClick(citation, button);
              }
            }}
            dangerouslySetInnerHTML={{ __html: richMarkdown.html }}
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
                  <span dangerouslySetInnerHTML={{ __html: blockHtml }} />
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
                  dangerouslySetInnerHTML={{ __html: markdownToInlineHtml(token.text) }}
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

  const rawContent = message.content || (message.pending ? "..." : "");
  const looksLikeHtml = /^\s*<(!doctype\s+html|html)/iu.test(rawContent);
  const trailingCitationsFallback =
    message.citations.length > 0 ? dedupeCitations(message.citations) : [];

  if (looksLikeHtml) {
    return (
      <>
        <div className={styles.markdownContent} dangerouslySetInnerHTML={{ __html: rawContent }} />
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
        dangerouslySetInnerHTML={{ __html: markdownToRichTextHtml(rawContent) }}
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

// =============================================================================
// Main Component
// =============================================================================

type ChatMessageListProps = {
  messages: ChatMessage[];
  progress: { activities: ProgressEntry[]; mode: WorkspaceChatMode | null; collapsed: boolean };
  isStreaming: boolean;
  locale: "zh-CN" | "en";
  onToggleProgressCollapsed: () => void;
  onSelectCitation: (request: WorkspaceCitationRequest) => void;
  onOpenWebSources: (request: WorkspaceWebSourcesRequest) => void;
  onCopyMessage: (content: string) => void;
  onEditMessage: (content: string) => void;
  onSubmitFeedback: (messageId: string, rating: "up" | "down") => void;
};

export function ChatMessageList({
  messages,
  progress,
  isStreaming,
  locale,
  onToggleProgressCollapsed,
  onSelectCitation,
  onOpenWebSources,
  onCopyMessage,
  onEditMessage,
  onSubmitFeedback,
}: ChatMessageListProps) {
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const [feedbackRatings, setFeedbackRatings] = useState<Record<string, "up" | "down">>({});

  // Auto-scroll to bottom on new messages / streaming
  useEffect(() => {
    const transcript = transcriptRef.current;
    if (!transcript) {
      return;
    }
    transcript.scrollTop = transcript.scrollHeight;
  }, [messages, isStreaming]);

  function handleCitationSelect(message: ChatMessage, citation: Citation, target?: HTMLElement | null) {
    if (message.sessionId && message.messageId !== null) {
      onSelectCitation({
        session_id: message.sessionId,
        message_id: message.messageId,
        citation,
        anchorRect: target ? getCitationAnchorRect(target) : null,
      });
    }
  }

  function handleFeedback(messageId: string, rating: "up" | "down") {
    setFeedbackRatings((prev) => ({ ...prev, [messageId]: rating }));
    onSubmitFeedback(messageId, rating);
  }

  return (
    <div
      className={styles.transcript}
      aria-label={formatUiMessage(locale, "workspaceTranscriptLabel")}
      ref={transcriptRef}
    >
      <div className={styles.transcriptInner}>
        {messages.map((message) => (
          <article
            className={[
              styles.message,
              message.role === "assistant" ? styles.messageAssistant : styles.messageUser,
            ]
              .filter(Boolean)
              .join(" ")}
            data-testid="chat-message"
            data-pending={message.pending}
            data-role={message.role}
            key={message.id}
          >
            <div
              className={[
                styles.messageContent,
                message.role === "assistant"
                  ? styles.messageContentAssistant
                  : styles.messageContentUser,
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
                  data-testid="mode-indicator"
                  data-mode={message.mode ?? "chat"}
                >
                  <span>{getModeLabel(locale, message.mode ?? "chat")}</span>
                  <span className={styles.modeBubbleTagCode}>
                    {getModeCode(message.mode ?? "chat")}
                  </span>
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

                {message.role === "assistant" && message.mode === "search" && !message.pending
                  ? (() => {
                      const webSources = collectWebSources(message.citations);
                      if (webSources.length === 0) {
                        return null;
                      }
                      return (
                        <button
                          className={styles.webSourceButton}
                          data-testid="citation-button"
                          onClick={() => onOpenWebSources({ sources: webSources })}
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
                        onCopyMessage(getCopyableMessageContent(message));
                      }
                      if (
                        message.role === "user" &&
                        action === formatUiMessage(locale, "workspaceChatActionEdit")
                      ) {
                        onEditMessage(message.content);
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
                        feedbackRatings[message.id] === "up" ? styles.messageActionButtonActive : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      disabled={feedbackRatings[message.id] === "up"}
                      type="button"
                      onClick={() => handleFeedback(message.id, "up")}
                      title={formatUiMessage(locale, "workspaceChatActionThumbUp")}
                    >
                      {feedbackRatings[message.id] === "up" ? "👍" : "🤙"}
                    </button>
                    <button
                      aria-label={formatUiMessage(locale, "workspaceChatActionThumbDown")}
                      className={[
                        styles.messageActionButton,
                        feedbackRatings[message.id] === "down"
                          ? styles.messageActionButtonActive
                          : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      disabled={feedbackRatings[message.id] === "down"}
                      type="button"
                      onClick={() => handleFeedback(message.id, "down")}
                      title={formatUiMessage(locale, "workspaceChatActionThumbDown")}
                    >
                      {feedbackRatings[message.id] === "down" ? "👎" : "🫣"}
                    </button>
                  </>
                ) : null}
              </div>

              {message.role === "assistant" && (message.guarded || message.degradeTrace.length > 0) ? (
                <div className={styles.messageNotice}>
                  {message.guarded ? (
                    <div className={styles.messageNoticeTitle}>
                      {formatUiMessage(locale, "workspaceGuardIntervened")}
                    </div>
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
        ))}

        {progress.mode ? (
          <ResearchProgressCard
            activities={progress.activities}
            collapsed={progress.collapsed}
            locale={locale}
            mode={progress.mode}
            onToggleCollapsed={onToggleProgressCollapsed}
          />
        ) : null}
      </div>
    </div>
  );
}
