"use client";

import type {
  WorkspaceCitationAnchor,
  WorkspaceWebSourcesRequest,
  WebSource,
} from "../../lib/workspace/model";
import {
  type AnswerBlock,
  type Citation,
} from "../../lib/workspace/stream";
import { markdownToInlineHtml, markdownToRichTextHtml } from "./workspace-note-rich-text";
import styles from "./workspace-chat.module.css";
import type { ChatMessage } from "../../hooks/use-chat-session";
import { ToolResultsPanel } from "./tool-result-card";

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

function getAnswerBlockText(blocks: AnswerBlock[]) {
  return blocks
    .filter((block): block is Extract<AnswerBlock, { type: "text" }> => block.type === "text")
    .map((block) => block.text)
    .join("");
}

export function getCitationAnchorRect(target: HTMLElement): WorkspaceCitationAnchor {
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
  const locator =
    citation.source_locator && typeof citation.source_locator === "object"
      ? (citation.source_locator as { url?: string | null })
      : null;
  const locatorUrl = locator?.url?.trim();
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


type CitationRendererProps = {
  locale: "zh-CN" | "en";
  message: ChatMessage;
  onOpenWebSources?: (request: WorkspaceWebSourcesRequest) => void;
  onSelectCitation: (citation: Citation, target: HTMLElement) => void;
};

export function CitationRenderer({
  locale,
  message,
  onOpenWebSources,
  onSelectCitation,
}: CitationRendererProps) {
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
