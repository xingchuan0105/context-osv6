"use client";

import { type MouseEvent, useEffect, useRef } from "react";
import type {
  WorkspaceWebSourcesRequest,
  WebSource,
} from "../../lib/workspace/model";
import {
  type AnswerBlock,
  type Citation,
} from "../../lib/workspace/stream";
import { formatUiMessage } from "../../lib/i18n/messages";
import { buildApiUrl } from "../../lib/http/request";
import { toSafeHttpUrl } from "../../lib/url/isSafeHttpUrl";
import { markdownToInlineHtml, markdownToRichTextHtml } from "./workspace-note-rich-text";
import { sanitizeWorkspaceHtml } from "./workspace-html-sanitize";
import styles from "./workspace-chat.module.css";
import type { UiChatMessage } from "../../hooks/use-chat-session";
import { ToolResultsPanel } from "./tool-result-card";

/** Resolve a displayable image URL for a citation (signed/http URL or asset API). */
function getCitationImageSrc(citation: Citation): string | null {
  const direct = citation.image_url?.trim();
  if (direct) {
    const safe = toSafeHttpUrl(direct) ?? (direct.startsWith("/") ? direct : null);
    if (safe) {
      return safe;
    }
    // Relative product paths (e.g. /api/v1/chat/citations/assets/…) are intentional.
    if (direct.startsWith("/")) {
      return direct;
    }
  }
  const assetId = citation.asset_id?.trim();
  if (assetId) {
    return buildApiUrl(`/api/v1/chat/citations/assets/${encodeURIComponent(assetId)}`);
  }
  return null;
}

type AnswerSegment =
  | { kind: "text"; text: string }
  | { kind: "image"; chunkId: string };

/**
 * Split answer markup so `[[image:chunk_id]]` becomes a first-class image segment
 * (rendered as an <img>, not a click-to-open citation chip).
 */
function splitAnswerSegments(markdown: string): AnswerSegment[] {
  const segments: AnswerSegment[] = [];
  const re = /\[\[image:([^\]]+)\]\]/giu;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = re.exec(markdown)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ kind: "text", text: markdown.slice(lastIndex, match.index) });
    }
    const chunkId = match[1]?.trim() ?? "";
    if (chunkId) {
      segments.push({ kind: "image", chunkId });
    }
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < markdown.length) {
    segments.push({ kind: "text", text: markdown.slice(lastIndex) });
  }
  if (segments.length === 0 && markdown.length > 0) {
    segments.push({ kind: "text", text: markdown });
  }
  return segments;
}

function getCitationLabel(citation: Citation, index: number) {
  return citation.doc_name.trim().length > 0 ? citation.doc_name : `Source ${index + 1}`;
}

function getCitationDisplayId(_citation: Citation, index: number) {
  // Always show 1-based appearance order in the answer (1,2,3…), not web observation ids.
  return String(index + 1);
}

function citationIdentityKey(citation: Citation): string {
  if (citation.chunk_id?.trim()) {
    return `doc:${citation.chunk_id.trim()}`;
  }
  const url = getCitationUrl(citation);
  if (url) {
    return `web:${url}`;
  }
  return `id:${citation.citation_id}:${citation.doc_id}`;
}

function getCitationPageText(locale: "zh-CN" | "en", page: number | null | undefined) {
  if (page === null || page === undefined) {
    return "";
  }
  return formatUiMessage(locale, "workspaceRightRail.viewerPage", { page: String(page) });
}

function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
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
  if (pageLabel) {
    return formatUiMessage(locale, "workspaceCitationAriaLabelWithPage", {
      displayId,
      label,
      pageLabel,
    });
  }
  return formatUiMessage(locale, "workspaceCitationAriaLabel", { displayId, label });
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
  const locator =
    citation.source_locator && typeof citation.source_locator === "object" && !Array.isArray(citation.source_locator)
      ? (citation.source_locator as { url?: string | null })
      : null;
  const locatorUrl = toSafeHttpUrl(locator?.url);
  if (locatorUrl) {
    return locatorUrl;
  }
  const docId = citation.doc_id.trim();
  return toSafeHttpUrl(docId);
}

function hasOnlyTextAnswerBlocks(blocks: AnswerBlock[]) {
  return (
    blocks.length > 0 &&
    blocks.every((block) => block.type === "text" && block.citations.length === 0)
  );
}

export function collectWebSources(citations: Citation[]): WebSource[] {
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
  // Doc: [[cite:CHUNK_ID]] / [[image:CHUNK_ID]]; Web: [[web:n]]; legacy: [[n]] / [n]
  return /\[\[(?:cite|image|web):[^\]]+\]\]|\[\[\d+\]\]|\[\d+\]/iu.test(content);
}

function resolveCitationFromMarker(
  citations: Citation[],
  opts: { displayId?: string; chunkId?: string },
): Citation | null {
  if (opts.chunkId) {
    const byChunk = findCitationByChunkId(citations, opts.chunkId);
    if (byChunk) {
      return byChunk;
    }
  }
  if (opts.displayId) {
    return findCitationByDisplayId(citations, opts.displayId);
  }
  return null;
}

type RichMarkdownCitationToken = {
  citation: Citation | null;
  token: string;
  /** Sequential chip number in answer order (1,2,3…). */
  displaySeq: number;
  /** Unresolved web marker — render non-clickable chip after markdown escape. */
  fallbackOnly?: boolean;
};

/**
 * Text-only citation chips. Image markers (`[[image:…]]`) are handled outside this
 * helper and rendered as real <img> figures (not click-to-open chips).
 */
function markdownToRichTextHtmlWithCitationButtons(
  markdown: string,
  citations: Citation[],
  locale: "zh-CN" | "en",
) {
  const citationTokens: RichMarkdownCitationToken[] = [];
  /** First-appearance order → sequential chip 1,2,3… (same source reuses same number). */
  const sequentialByKey = new Map<string, number>();
  let nextSequential = 1;

  const allocSeq = (key: string) => {
    let seq = sequentialByKey.get(key);
    if (seq === undefined) {
      seq = nextSequential;
      nextSequential += 1;
      sequentialByKey.set(key, seq);
    }
    return seq;
  };

  // Placeholders must be plain text so markdown/escapeHtml does not mangle them;
  // real <button>/<span> HTML is injected only after markdownToRichTextHtml.
  // Note: [[image:…]] is intentionally NOT matched here — images render as figures.
  const tokenizedMarkdown = markdown.replace(
    /\[\[cite:([^\]]+)\]\]|\[\[web:(\d+)\]\]|\[\[(\d+)\]\]|\[(?:web:|citation:)?\s*(\d+)\]/giu,
    (
      _marker,
      citeChunkId: string | undefined,
      webId: string | undefined,
      bracketedId: string | undefined,
      prefixedId: string | undefined,
    ) => {
      const citation = resolveCitationFromMarker(citations, {
        chunkId: citeChunkId,
        displayId: webId ?? bracketedId ?? prefixedId,
      });
      if (!citation) {
        const fallbackId = webId ?? bracketedId ?? prefixedId;
        if (!fallbackId) {
          return "";
        }
        const seq = allocSeq(`fallback:${fallbackId}`);
        const token = `CITATIONTOKEN${citationTokens.length}END`;
        citationTokens.push({
          citation: null,
          token,
          displaySeq: seq,
          fallbackOnly: true,
        });
        return token;
      }
      const seq = allocSeq(citationIdentityKey(citation));
      const token = `CITATIONTOKEN${citationTokens.length}END`;
      citationTokens.push({ citation, token, displaySeq: seq });
      return token;
    },
  );
  let html = markdownToRichTextHtml(tokenizedMarkdown);

  citationTokens.forEach(({ citation, token, displaySeq, fallbackOnly }, tokenIndex) => {
    const displayId = escapeHtmlAttribute(String(displaySeq));
    if (fallbackOnly || !citation) {
      const spanHtml = `<span class="${styles.inlineCitationFallback}" title="source">${displayId}</span>`;
      html = html.split(token).join(spanHtml);
      return;
    }
    const citationIndex = findCitationIndex(citations, citation);
    const resolvedIndex = citationIndex >= 0 ? citationIndex : 0;
    const label = escapeHtmlAttribute(getInlineCitationAriaLabel(locale, citation, resolvedIndex));
    const buttonHtml = `<button aria-label="${label}" class="${styles.inlineCitationButton}" data-inline-citation-token-index="${tokenIndex}" data-testid="workspace-citation" type="button">${displayId}</button>`;
    html = html.split(token).join(buttonHtml);
  });

  return { citationTokens, html };
}

// =============================================================================
// Sub-components
// =============================================================================

/** Wrap rendered code blocks with a language label + copy button (done in JS so
 *  sanitize-safe markdown HTML stays unchanged). */
function enhanceCodeBlocks(container: HTMLElement, locale: "zh-CN" | "en") {
  container.querySelectorAll("pre").forEach((pre) => {
    const existingWrapper = pre.parentElement;
    if (existingWrapper?.classList.contains(styles.codeBlockWrapper)) {
      // Already wrapped (e.g. locale switched): refresh the copy button label
      // instead of skipping, so the text follows the active locale.
      const existingButton = existingWrapper.querySelector<HTMLButtonElement>(
        `.${styles.codeBlockCopyButton}`,
      );
      if (existingButton && existingButton.dataset.copied !== "true") {
        existingButton.textContent = formatUiMessage(locale, "workspaceChatCodeCopy");
      }
      return;
    }

    const wrapper = document.createElement("div");
    wrapper.className = styles.codeBlockWrapper;
    pre.parentNode?.insertBefore(wrapper, pre);
    wrapper.appendChild(pre);

    const language = pre.getAttribute("data-language")?.trim();
    if (language) {
      const label = document.createElement("span");
      label.className = styles.codeBlockLanguage;
      label.textContent = language;
      wrapper.appendChild(label);
    }

    const copyButton = document.createElement("button");
    copyButton.type = "button";
    copyButton.className = styles.codeBlockCopyButton;
    copyButton.textContent = formatUiMessage(locale, "workspaceChatCodeCopy");
    copyButton.addEventListener("click", () => {
      if (typeof navigator === "undefined" || !navigator.clipboard) {
        return;
      }
      void navigator.clipboard.writeText(pre.textContent ?? "").then(() => {
        copyButton.dataset.copied = "true";
        copyButton.textContent = formatUiMessage(locale, "workspaceChatCodeCopied");
        window.setTimeout(() => {
          copyButton.dataset.copied = "false";
          copyButton.textContent = formatUiMessage(locale, "workspaceChatCodeCopy");
        }, 1500);
      });
    });
    wrapper.appendChild(copyButton);
  });
}

function MarkdownContent({
  html,
  locale,
  onClick,
}: {
  html: string;
  locale: "zh-CN" | "en";
  onClick?: (event: MouseEvent<HTMLDivElement>) => void;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (containerRef.current) {
      enhanceCodeBlocks(containerRef.current, locale);
    }
  }, [html, locale]);

  return (
    <div
      className={styles.markdownContent}
      onClick={onClick}
      ref={containerRef}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}


type CitationRendererProps = {
  locale: "zh-CN" | "en";
  message: UiChatMessage;
  onOpenWebSources?: (request: WorkspaceWebSourcesRequest) => void;
  onSelectCitation: (citation: Citation) => void;
};

export function CitationRenderer({
  locale,
  message,
  onOpenWebSources,
  onSelectCitation,
}: CitationRendererProps) {
  function handleCitationClick(citation: Citation) {
    const hasSearch =
      message.capabilities?.includes("search") ||
      message.mode === "search" ||
      message.mode === "rag+search";
    const webSource = hasSearch ? citationToWebSource(citation) : null;
    if (webSource && onOpenWebSources) {
      onOpenWebSources({ sources: [webSource] });
      return;
    }
    onSelectCitation(citation);
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
        onClick={() => {
          handleCitationClick(citation);
        }}
        title={hoverTitle.slice(0, 300)}
        type="button"
      >
        {getCitationDisplayId(citation, resolvedIndex)}
      </button>
    );
  }

  /**
   * Inline figure: image is always shown (no click-to-reveal). Optional click
   * opens citation details; does not gate visibility.
   */
  function renderImageCard(citation: Citation, key: string) {
    const citationIndex = findCitationIndex(message.citations, citation);
    const resolvedIndex = citationIndex >= 0 ? citationIndex : 0;
    const imageSrc = getCitationImageSrc(citation);
    const caption = citation.caption?.trim() || getCitationLabel(citation, resolvedIndex);
    const aria = getInlineCitationAriaLabel(locale, citation, resolvedIndex);

    return (
      <figure
        aria-label={aria}
        className={styles.answerImageCard}
        data-testid="workspace-answer-image"
        key={key}
        onClick={() => {
          onSelectCitation(citation);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelectCitation(citation);
          }
        }}
        role="button"
        tabIndex={0}
      >
        {imageSrc ? (
          // eslint-disable-next-line @next/next/no-img-element -- citation asset URLs are dynamic API/store paths
          <img alt={caption} className={styles.answerImage} loading="lazy" src={imageSrc} />
        ) : (
          <span className={styles.answerImageFallback}>
            {formatUiMessage(locale, "workspaceCitationImageUnavailable")}
          </span>
        )}
        <figcaption className={styles.answerImageMeta}>
          <span className={styles.answerImageBadge}>
            {getCitationDisplayId(citation, resolvedIndex)}
          </span>
          <span className={styles.answerImageCaption}>{caption}</span>
        </figcaption>
      </figure>
    );
  }

  /** Markdown answer with `[[image:chunk]]` expanded to visible figures in-flow. */
  function renderSegmentedMarkdown(markdown: string, keyPrefix: string) {
    const segments = splitAnswerSegments(markdown);
    const citationTokensAll: RichMarkdownCitationToken[] = [];

    const nodes = segments.map((segment, segmentIndex) => {
      if (segment.kind === "image") {
        const citation = findCitationByChunkId(message.citations, segment.chunkId);
        if (!citation) {
          return null;
        }
        return renderImageCard(citation, `${keyPrefix}-img-${segmentIndex}`);
      }

      const trimmed = segment.text.trim();
      if (!trimmed) {
        return null;
      }
      const richMarkdown = markdownToRichTextHtmlWithCitationButtons(
        segment.text,
        message.citations,
        locale,
      );
      citationTokensAll.push(...richMarkdown.citationTokens);

      return (
        <MarkdownContent
          html={sanitizeWorkspaceHtml(richMarkdown.html)}
          key={`${keyPrefix}-text-${segmentIndex}`}
          locale={locale}
          onClick={(event) => {
            const target = event.target as HTMLElement;
            const button = target.closest<HTMLButtonElement>(
              "button[data-inline-citation-token-index]",
            );
            if (!button) {
              return;
            }
            const localIndex = Number.parseInt(button.dataset.inlineCitationTokenIndex ?? "", 10);
            if (Number.isNaN(localIndex)) {
              return;
            }
            // Per-segment token indices (0..n-1) for this MarkdownContent only.
            const citation = richMarkdown.citationTokens[localIndex]?.citation ?? null;
            if (citation) {
              handleCitationClick(citation);
            }
          }}
        />
      );
    });

    const hasAnyImage = segments.some((s) => s.kind === "image");
    const resolvedCitationCount = citationTokensAll.filter((t) => t.citation).length;
    const trailingCitations =
      !hasAnyImage &&
      resolvedCitationCount === 0 &&
      message.citations.length > 0
        ? dedupeCitations(message.citations)
        : [];

    return (
      <>
        <div className={styles.answerBlockStack}>{nodes}</div>
        {trailingCitations.length > 0 ? (
          <div className={`${styles.inlineCitationGroup} ${styles.inlineCitationGroupTrailing}`}>
            {trailingCitations.map((citation, idx) =>
              renderCitationButton(citation, `${keyPrefix}-trailing-${idx}`),
            )}
          </div>
        ) : null}
      </>
    );
  }

  if (message.pending) {
    // Streaming in progress: lightweight plain-text render — skip full markdown
    // parsing and citation buttons until the message completes.
    const streamText =
      message.answerBlocks.length > 0
        ? getAnswerBlockText(message.answerBlocks)
        : message.content;

    return (
      <>
        <div className={styles.streamingPlaintext}>{streamText || "..."}</div>
        <ToolResultsPanel locale={locale} results={message.toolResults} />
      </>
    );
  }

  if (message.answerBlocks.length > 0) {
    if (hasOnlyTextAnswerBlocks(message.answerBlocks)) {
      // Prefer full message.content when it still carries [[image:]] / cite markup
      // (answer_blocks text-only merge drops image markers).
      const sourceText = hasRenderedCitationMarkup(message.content)
        ? message.content
        : getAnswerBlockText(message.answerBlocks);
      return (
        <>
          {renderSegmentedMarkdown(sourceText, "blocks-text")}
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

            // Text blocks may still embed [[image:]] if markup was inlined in text.
            if (/\[\[image:/iu.test(block.text)) {
              return (
                <div key={`text-seg-${blockIndex}`}>
                  {renderSegmentedMarkdown(block.text, `block-${blockIndex}`)}
                </div>
              );
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
                  <span dangerouslySetInnerHTML={{ __html: sanitizeWorkspaceHtml(blockHtml) }} />
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
    return (
      <>
        {renderSegmentedMarkdown(message.content, "content")}
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
        <MarkdownContent html={sanitizeWorkspaceHtml(rawContent)} locale={locale} />
        {trailingCitationsFallback.length > 0 ? (
          <div className={`${styles.inlineCitationGroup} ${styles.inlineCitationGroupTrailing}`}>
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
      <MarkdownContent html={sanitizeWorkspaceHtml(markdownToRichTextHtml(rawContent))} locale={locale} />
      {trailingCitationsFallback.length > 0 ? (
        <div className={`${styles.inlineCitationGroup} ${styles.inlineCitationGroupTrailing}`}>
          {trailingCitationsFallback.map((citation, idx) =>
            renderCitationButton(citation, `trailing-${idx}`),
          )}
        </div>
      ) : null}
      <ToolResultsPanel locale={locale} results={message.toolResults} />
    </>
  );
}
