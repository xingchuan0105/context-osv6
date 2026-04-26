"use client";

import Link from "next/link";
import { type FormEvent, useEffect, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import {
  getSharedWorkspace,
  streamSharedChat,
  type SharedWorkspacePayload,
} from "../../lib/share/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  type AnswerBlock,
  type Citation,
  type SourceRef,
  type WorkspaceChatStreamEvent,
} from "../../lib/workspace/stream";
import styles from "./shared-workspace-surface.module.css";

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

function getCitationLabel(citation: Citation, index: number) {
  return citation.doc_name.trim().length > 0 ? citation.doc_name : `citation-${index + 1}`;
}

function normalizeSemanticValue(value: string | null | undefined) {
  const normalized = value?.trim().toLowerCase();

  return normalized && normalized.length > 0 ? normalized : "unknown";
}

function dedupeSources(sources: SourceRef[]) {
  const seen = new Set<string>();

  return sources.filter((source) => {
    const key = source.id.trim() || `${source.doc_id ?? ""}:${source.page ?? ""}:${source.title.trim()}`;

    if (!key || seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

function sourcesFromCitations(citations: Citation[]) {
  return dedupeSources(
    citations.map((citation, index) => ({
      id: citation.chunk_id?.trim() || citation.doc_id.trim() || `citation-source-${index}`,
      title: getCitationLabel(citation, index),
      snippet: citation.preview?.trim() || citation.content?.trim() || null,
      doc_id: citation.doc_id,
      page: citation.page ?? null,
    })),
  );
}

function buildPromptSuggestions(payload: SharedWorkspacePayload, fallback: string) {
  const title = payload.knowledge_base.title.trim();
  const sourceNames = payload.sources
    .map((source) => source.file_name.trim())
    .filter((value) => value.length > 0);

  const suggestions = [title ? `${title}?` : "", sourceNames[0] ? `${sourceNames[0]}?` : "", [title, sourceNames[0]].filter(Boolean).join(" / ")]
    .map((value) => value.trim())
    .filter((value) => value.length > 0);

  const uniqueSuggestions = [...new Set(suggestions)];

  return uniqueSuggestions.length > 0 ? uniqueSuggestions.slice(0, 3) : [fallback];
}

function loadErrorSemantic(error: string, shareToken: string) {
  if (!shareToken.trim()) {
    return "invalid";
  }

  const normalized = error.trim().toLowerCase();

  if (!normalized) {
    return "invalid";
  }

  if (normalized.includes("expired")) {
    return "expired";
  }

  if (normalized.includes("invalid")) {
    return "invalid";
  }

  return normalized;
}

export function SharedWorkspaceSurface({ shareToken }: { shareToken: string }) {
  const { locale } = useUiPreferences();
  const auth = useAuth();
  const [payload, setPayload] = useState<SharedWorkspacePayload | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [chatError, setChatError] = useState("");
  const [query, setQuery] = useState("");
  const [answer, setAnswer] = useState("");
  const [streamingAnswer, setStreamingAnswer] = useState("");
  const [citations, setCitations] = useState<Citation[]>([]);
  const [sources, setSources] = useState<SourceRef[]>([]);
  const [degradeReasons, setDegradeReasons] = useState<string[]>([]);
  const [answering, setAnswering] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function loadSharedWorkspace() {
      if (!shareToken.trim()) {
        setLoadError("invalid");
        setLoading(false);
        return;
      }

      setLoading(true);
      setLoadError("");

      try {
        const response = await getSharedWorkspace(shareToken);

        if (!cancelled) {
          setPayload(response);
        }
      } catch (loadFailure) {
        if (!cancelled) {
          setLoadError(loadFailure instanceof Error ? loadFailure.message : "invalid");
          setPayload(null);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void loadSharedWorkspace();

    return () => {
      cancelled = true;
    };
  }, [shareToken]);

  function handleStreamEvent(event: WorkspaceChatStreamEvent) {
    switch (event.kind) {
      case "token":
        setStreamingAnswer((current) => `${current}${event.content}`);
        break;
      case "citations":
        setCitations(event.citations);
        setSources(sourcesFromCitations(event.citations));
        break;
      case "done": {
        const nextCitations = event.payload.citations ?? [];
        const nextSources =
          event.payload.sources && event.payload.sources.length > 0
            ? dedupeSources(event.payload.sources)
            : sourcesFromCitations(nextCitations);

        setAnswer(getAnswerText(event.payload.answer ?? "", event.payload.answer_blocks ?? []));
        setStreamingAnswer("");
        setCitations(nextCitations);
        setSources(nextSources);
        setDegradeReasons((event.payload.degrade_trace ?? []).map((item) => item.reason).filter(Boolean));
        setAnswering(false);
        break;
      }
      case "error":
        setChatError(event.message);
        setStreamingAnswer("");
        setAnswering(false);
        break;
      case "activity":
      case "answer_start":
      case "start":
      case "trace":
        break;
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const nextQuery = query.trim();

    if (!payload || !nextQuery || answering) {
      return;
    }

    if (!auth.token) {
      setChatError(formatUiMessage(locale, "sharedPublic.signInRequiredBody"));
      return;
    }

    setAnswering(true);
    setChatError("");
    setAnswer("");
    setStreamingAnswer("");
    setCitations([]);
    setSources([]);
    setDegradeReasons([]);

    try {
      await streamSharedChat(
        shareToken,
        payload.knowledge_base.id,
        nextQuery,
        handleStreamEvent,
        auth.token,
      );
    } catch (submitFailure) {
      setChatError(
        submitFailure instanceof Error
          ? submitFailure.message
          : formatUiMessage(locale, "sharedPublic.signInRequiredBody"),
      );
      setAnswering(false);
    }
  }

  const answerText = streamingAnswer || answer;
  const readySourceCount = payload?.sources.filter((source) => matches(source.status)).length ?? 0;
  const pendingSourceCount = payload ? payload.sources.length - readySourceCount : 0;
  const canInteract = auth.initialized && Boolean(auth.token);
  const nextPath = `/shared/kb/${shareToken}`;
  const promptSuggestions = payload
    ? buildPromptSuggestions(payload, formatUiMessage(locale, "sharedPublic.questionPlaceholder"))
    : [];

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem" }}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <Link className="app-link app-link-muted" href="/">
            {formatUiMessage(locale, "sharedPublic.backHomeAction")}
          </Link>
          <div>
            <h1 className="app-page-title">{payload?.knowledge_base.title ?? formatUiMessage(locale, "sharedPublic.pageTitle")}</h1>
            <p className="app-page-subtitle">
              {payload?.knowledge_base.description?.trim() || formatUiMessage(locale, "sharedPublic.pageSubtitle")}
            </p>
          </div>
        </header>

        {loading ? (
          <section className="app-surface-card" role="status">
            <p style={{ margin: 0 }}>{formatUiMessage(locale, "sharedPublic.loading")}</p>
          </section>
        ) : loadError || !payload ? (
          <section className="app-surface-card" style={{ display: "grid", gap: "0.75rem" }}>
            <h2 className="app-page-title" style={{ fontSize: "1.15rem", marginBottom: 0 }}>
              {formatUiMessage(locale, "sharedPublic.invalidLinkTitle")}
            </h2>
            <p className="app-page-subtitle">{formatUiMessage(locale, "sharedPublic.invalidLinkBody")}</p>
            <code className={styles.semanticCode}>{loadErrorSemantic(loadError, shareToken)}</code>
            {loadError && loadErrorSemantic(loadError, shareToken) !== loadError.trim().toLowerCase() ? (
              <code className={styles.semanticCode}>{loadError}</code>
            ) : null}
          </section>
        ) : (
          <>
            <section className={`app-surface-card ${styles.sectionStack}`}>
              <div className={styles.overviewHeader}>
                <div>
                  <h2 className="app-page-title" style={{ fontSize: "1.3rem", marginBottom: "0.5rem" }}>
                    {payload.knowledge_base.title}
                  </h2>
                  <p className="app-page-subtitle">{payload.knowledge_base.description?.trim() || formatUiMessage(locale, "sharedPublic.pageSubtitle")}</p>
                </div>
                <div className={styles.overviewMeta}>
                  <span className={styles.metaPair}>
                    <span className={styles.metaLabel}>{formatUiMessage(locale, "sharedPublic.expiresAtLabel")}</span>
                    <span className={styles.metaValue}>{String(payload.share.expires_at ?? "null")}</span>
                  </span>
                  <span className={styles.metaPair}>
                    <span className={styles.metaLabel}>{formatUiMessage(locale, "sharedPublic.sourcesSectionTitle")}</span>
                    <span className={styles.metaValue}>{payload.sources.length}</span>
                  </span>
                </div>
              </div>

              <div className={styles.metricGrid}>
                <article className={styles.metricCard}>
                  <div className={styles.metricLabel}>{formatUiMessage(locale, "sharedPublic.readAccessLabel")}</div>
                  <div className={styles.metricValueCompact}>
                    {formatUiMessage(locale, "sharedPublic.readAccessValue")}
                  </div>
                </article>
                <article className={styles.metricCard}>
                  <div className={styles.metricLabel}>{formatUiMessage(locale, "sharedPublic.interactionAccessLabel")}</div>
                  <div className={styles.metricValueCompact}>
                    {formatUiMessage(locale, "sharedPublic.interactionAccessValue")}
                  </div>
                </article>
                <article className={styles.metricCard}>
                  <div className={styles.metricLabel}>{formatUiMessage(locale, "sharedPublic.downloadPolicyLabel")}</div>
                  <div className={styles.metricValueCompact}>
                    {payload.share.allow_download
                      ? formatUiMessage(locale, "sharedPublic.downloadAllowed")
                      : formatUiMessage(locale, "sharedPublic.downloadOnlineOnly")}
                  </div>
                </article>
                <article className={styles.metricCard}>
                  <div className={styles.metricLabel}>{formatUiMessage(locale, "sharedPublic.sourcesSectionTitle")}</div>
                  <div className={styles.metricValue}>{payload.sources.length}</div>
                  <div className={styles.metricValueCompact}>{`${readySourceCount} / ${pendingSourceCount}`}</div>
                </article>
              </div>
            </section>

            <section className={`app-surface-card ${styles.sectionStack}`}>
              <div>
                <h2 className="app-page-title" style={{ fontSize: "1.3rem", marginBottom: "0.5rem" }}>
                  {formatUiMessage(locale, "sharedPublic.sourcesSectionTitle")}
                </h2>
                <p className="app-page-subtitle">{formatUiMessage(locale, "sharedPublic.sourcesSectionSubtitle")}</p>
              </div>

              {payload.sources.length === 0 ? (
                <div className={styles.emptyState}>
                  <strong>{formatUiMessage(locale, "sharedPublic.sourcesEmptyTitle")}</strong>
                  <p style={{ margin: 0 }}>{formatUiMessage(locale, "sharedPublic.sourcesEmptyBody")}</p>
                </div>
              ) : (
                <div className={styles.sourceList}>
                  {payload.sources.map((source) => (
                    <article className={styles.sourceCard} key={source.id}>
                      <div className={styles.sourceTitleRow}>
                        <strong>{source.file_name}</strong>
                        <code className={styles.semanticCode}>{normalizeSemanticValue(source.status)}</code>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>

            <section className={`app-surface-card ${styles.sectionStack}`}>
              <div>
                <h2 className="app-page-title" style={{ fontSize: "1.3rem", marginBottom: "0.5rem" }}>
                  {formatUiMessage(locale, "sharedPublic.chatSectionTitle")}
                </h2>
                <p className="app-page-subtitle">{formatUiMessage(locale, "sharedPublic.chatSectionSubtitle")}</p>
              </div>

              {canInteract && promptSuggestions.length > 0 ? (
                <div className={styles.suggestionRow}>
                  {promptSuggestions.map((suggestion) => (
                    <button
                      className={styles.suggestionChip}
                      key={suggestion}
                      onClick={() => setQuery(suggestion)}
                      type="button"
                    >
                      {suggestion}
                    </button>
                  ))}
                </div>
              ) : null}

              {chatError ? <p className="app-notice-banner">{chatError}</p> : null}

              {canInteract ? (
                <form onSubmit={handleSubmit} style={{ display: "grid", gap: "1rem" }}>
                  <div>
                    <label className="app-form-label" htmlFor="shared-query">
                      {formatUiMessage(locale, "sharedPublic.questionLabel")}
                    </label>
                    <textarea
                      className="app-input"
                      id="shared-query"
                      onChange={(event) => setQuery(event.target.value)}
                      placeholder={formatUiMessage(locale, "sharedPublic.questionPlaceholder")}
                      rows={4}
                      value={query}
                    />
                  </div>
                  <div className="app-button-row">
                    <button className="app-button-primary" disabled={answering || query.trim().length === 0} type="submit">
                      {answering ? formatUiMessage(locale, "sharedPublic.submitting") : formatUiMessage(locale, "sharedPublic.submitAction")}
                    </button>
                  </div>
                </form>
              ) : (
                <div className="app-inline-surface" style={{ display: "grid", gap: "0.75rem" }}>
                  <strong>{formatUiMessage(locale, "sharedPublic.signInRequiredTitle")}</strong>
                  <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
                    {formatUiMessage(locale, "sharedPublic.signInRequiredBody")}
                  </p>
                  <div className="app-button-row">
                    <Link className="app-button-primary" href={`/login?next=${encodeURIComponent(nextPath)}`}>
                      {formatUiMessage(locale, "sharedPublic.signInToContinueAction")}
                    </Link>
                    <Link className="app-button-secondary" href={`/register?next=${encodeURIComponent(nextPath)}`}>
                      {formatUiMessage(locale, "sharedPublic.signUpToContinueAction")}
                    </Link>
                  </div>
                </div>
              )}

              {degradeReasons.length > 0 ? (
                <div className={styles.degradedBanner} role="alert">
                  <strong>{formatUiMessage(locale, "sharedPublic.degradedBanner")}</strong>
                  <div className={styles.metricSplit}>
                    {degradeReasons.map((reason) => (
                      <code className={styles.semanticCode} key={reason}>
                        {reason}
                      </code>
                    ))}
                  </div>
                </div>
              ) : null}

              {answerText || citations.length > 0 || sources.length > 0 ? (
                <div className={styles.resultStack}>
                  {answerText ? (
                    <section className={styles.resultCard}>
                      <div className={styles.resultHeader}>
                        <h3 style={{ margin: 0 }}>{formatUiMessage(locale, "sharedPublic.answerTitle")}</h3>
                      </div>
                      <p className={styles.answerCopy}>{answerText}</p>
                    </section>
                  ) : null}

                  {citations.length > 0 ? (
                    <section className={styles.resultCard}>
                      <div className={styles.resultHeader}>
                        <h3 style={{ margin: 0 }}>{formatUiMessage(locale, "sharedPublic.citationsTitle")}</h3>
                        <span className={styles.resultCount}>{citations.length}</span>
                      </div>
                      <div className={styles.resultList}>
                        {citations.map((citation, index) => (
                          <article className={styles.resultItem} key={`${citation.doc_id}-${citation.citation_id || index}`}>
                            <div className={styles.sourceTitleRow}>
                              <strong>{getCitationLabel(citation, index)}</strong>
                              {citation.page ? <code className={styles.semanticCode}>page={citation.page}</code> : null}
                            </div>
                            {citation.preview?.trim() ? <p className={styles.previewCopy}>{citation.preview}</p> : null}
                          </article>
                        ))}
                      </div>
                    </section>
                  ) : null}

                  {sources.length > 0 ? (
                    <section className={styles.resultCard}>
                      <div className={styles.resultHeader}>
                        <h3 style={{ margin: 0 }}>{formatUiMessage(locale, "workspaceRightRail.sourcesSectionTitle")}</h3>
                        <span className={styles.resultCount}>{sources.length}</span>
                      </div>
                      <div className={styles.resultList}>
                        {sources.map((source) => (
                          <article className={styles.resultItem} key={source.id}>
                            <div className={styles.sourceTitleRow}>
                              <strong>{source.title}</strong>
                              {source.page ? <code className={styles.semanticCode}>page={source.page}</code> : null}
                            </div>
                            {source.snippet?.trim() ? <p className={styles.previewCopy}>{source.snippet}</p> : null}
                          </article>
                        ))}
                      </div>
                    </section>
                  ) : null}
                </div>
              ) : null}
            </section>
          </>
        )}
      </div>
    </main>
  );
}

function matches(status: string) {
  const normalized = normalizeSemanticValue(status);

  return normalized === "ready" || normalized === "completed";
}
