"use client";

import Link from "next/link";
import { type FormEvent, useEffect, useRef, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import {
  getSharedWorkspace,
  streamSharedChat,
  type SharedWorkspacePayload,
} from "../../lib/share/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import type { ChatResponse } from "../../lib/contracts";
import { userVisibleDegradeReasons } from "../../lib/workspace/degrade-display";
import {
  type AnswerBlock,
  type Citation,
  parseStreamCitations,
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
      snippet: citation.preview?.trim() || citation.content?.trim() || undefined,
      doc_id: citation.doc_id,
      page: citation.page ?? undefined,
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

const TURNSTILE_SITE_KEY =
  typeof process !== "undefined"
    ? (process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY ?? "").trim()
    : "";

declare global {
  interface Window {
    turnstile?: {
      render: (
        el: HTMLElement,
        opts: {
          sitekey: string;
          callback: (token: string) => void;
          "expired-callback"?: () => void;
          "error-callback"?: () => void;
        },
      ) => string;
      reset: (widgetId?: string) => void;
    };
  }
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
  const [turnstileToken, setTurnstileToken] = useState("");
  const turnstileRef = useRef<HTMLDivElement | null>(null);
  const turnstileWidgetId = useRef<string | null>(null);
  const [streamingAnswer, setStreamingAnswer] = useState("");
  const [citations, setCitations] = useState<Citation[]>([]);
  const [sources, setSources] = useState<SourceRef[]>([]);
  const [degradeReasons, setDegradeReasons] = useState<string[]>([]);
  const [answering, setAnswering] = useState(false);

  useEffect(() => {
    if (!TURNSTILE_SITE_KEY || !payload) return;
    let cancelled = false;
    const mount = () => {
      if (cancelled || !turnstileRef.current || !window.turnstile) return;
      if (turnstileWidgetId.current) return;
      turnstileWidgetId.current = window.turnstile.render(turnstileRef.current, {
        sitekey: TURNSTILE_SITE_KEY,
        callback: (token) => setTurnstileToken(token),
        "expired-callback": () => setTurnstileToken(""),
        "error-callback": () => setTurnstileToken(""),
      });
    };
    if (window.turnstile) {
      mount();
      return () => {
        cancelled = true;
      };
    }
    const existing = document.querySelector<HTMLScriptElement>(
      'script[src*="challenges.cloudflare.com/turnstile"]',
    );
    if (existing) {
      existing.addEventListener("load", mount);
      return () => {
        cancelled = true;
        existing.removeEventListener("load", mount);
      };
    }
    const script = document.createElement("script");
    script.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
    script.async = true;
    script.onload = mount;
    document.head.appendChild(script);
    return () => {
      cancelled = true;
    };
  }, [payload]);

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
    switch (event.event) {
      case "token":
        setStreamingAnswer((current) => `${current}${event.content}`);
        break;
      case "citations": {
        const parsedCitations = parseStreamCitations(event.citations);
        setCitations(parsedCitations);
        setSources(sourcesFromCitations(parsedCitations));
        break;
      }
      case "done": {
        const payload = event.payload as unknown as ChatResponse;
        const nextCitations = payload.citations ?? [];
        const nextSources =
          payload.sources && payload.sources.length > 0
            ? dedupeSources(payload.sources)
            : sourcesFromCitations(nextCitations);

        setAnswer(getAnswerText(payload.answer ?? "", payload.answer_blocks ?? []));
        setStreamingAnswer("");
        setCitations(nextCitations);
        setSources(nextSources);
        setDegradeReasons(
          userVisibleDegradeReasons(
            (payload.degrade_trace ?? []).map((item) => item.reason).filter(Boolean),
          ),
        );
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

    // ADR-0010: do not require auth.token client-side. Backend allows anonymous
    // when workspace visibility is `public`; otherwise returns login_required.
    setAnswering(true);
    setChatError("");
    setAnswer("");
    setStreamingAnswer("");
    setCitations([]);
    setSources([]);
    setDegradeReasons([]);

    try {
      if (TURNSTILE_SITE_KEY && !turnstileToken.trim() && !auth.token) {
        setChatError(
          locale === "zh-CN"
            ? "请先完成人机验证后再提问。"
            : "Complete the human verification challenge before asking.",
        );
        setAnswering(false);
        return;
      }
      await streamSharedChat(
        shareToken,
        payload.knowledge_base.id,
        nextQuery,
        handleStreamEvent,
        auth.token ?? null,
        turnstileToken || null,
      );
      if (TURNSTILE_SITE_KEY && turnstileWidgetId.current && window.turnstile) {
        window.turnstile.reset(turnstileWidgetId.current);
        setTurnstileToken("");
      }
    } catch (submitFailure) {
      const msg =
        submitFailure instanceof Error
          ? submitFailure.message
          : formatUiMessage(locale, "sharedPublic.signInRequiredBody");
      setChatError(msg);
      setAnswering(false);
    }
  }

  const answerText = streamingAnswer || answer;
  const readySourceCount = payload?.sources.filter((source) => matches(source.status)).length ?? 0;
  const pendingSourceCount = payload ? payload.sources.length - readySourceCount : 0;
  // Loaded shared page can chat; login only if backend rejects (link mode).
  const canInteract = Boolean(payload);
  const nextPath = `/shared/kb/${shareToken}`;
  const promptSuggestions = payload
    ? buildPromptSuggestions(payload, formatUiMessage(locale, "sharedPublic.questionPlaceholder"))
    : [];

  const owner = payload?.owner ?? null;
  const ownerAvatar = owner?.avatar_url?.trim()
    ? owner.avatar_url.startsWith("http")
      ? owner.avatar_url
      : owner.avatar_url
    : null;
  const ownerBanner = owner?.banner_url?.trim()
    ? owner.banner_url.startsWith("http")
      ? owner.banner_url
      : owner.banner_url
    : null;

  return (
    <main className="app-page-shell">
      <div className={`app-page-center ${styles.pageStack}`}>
        <header className={styles.header}>
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
            <p className={styles.flushText}>{formatUiMessage(locale, "sharedPublic.loading")}</p>
          </section>
        ) : loadError || !payload ? (
          <section className={`app-surface-card ${styles.errorCard}`}>
            <h2 className={`app-page-title ${styles.errorTitle}`}>
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
            {owner ? (
              <section
                aria-label={formatUiMessage(locale, "sharedPublic.ownerCardLabel")}
                className={`app-surface-card ${styles.ownerCard}`}
                data-testid="share-owner-card"
              >
                <div
                  className={styles.ownerBanner}
                  style={ownerBanner ? { backgroundImage: `url(${ownerBanner})` } : undefined}
                />
                <div className={styles.ownerBody}>
                  <div
                    className={styles.ownerAvatar}
                    style={ownerAvatar ? { backgroundImage: `url(${ownerAvatar})` } : undefined}
                    aria-hidden
                  >
                    {!ownerAvatar
                      ? owner.display_name.trim().slice(0, 1).toUpperCase() || "O"
                      : null}
                  </div>
                  <div className={styles.ownerMeta}>
                    <h2 className={styles.ownerName}>{owner.display_name}</h2>
                    {owner.bio?.trim() ? (
                      <p className={styles.ownerBio}>{owner.bio.trim()}</p>
                    ) : null}
                    {owner.contact_url?.trim() ? (
                      <a
                        className="app-link"
                        href={owner.contact_url.trim()}
                        rel="noreferrer"
                        target="_blank"
                      >
                        {formatUiMessage(locale, "sharedPublic.ownerContactAction")}
                      </a>
                    ) : null}
                  </div>
                </div>
              </section>
            ) : null}

            <section className={`app-surface-card ${styles.sectionStack}`}>
              <div className={styles.overviewHeader}>
                <div>
                  <h2 className={`app-page-title ${styles.sectionTitle}`}>
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

              <div className={styles.metricSplit}>
                <code className={styles.semanticCode}>{`permission=${normalizeSemanticValue(payload.share.permission)}`}</code>
                <code className={styles.semanticCode}>{`scope=${normalizeSemanticValue(payload.share.scope)}`}</code>
                <code className={styles.semanticCode}>{`allow_download=${String(payload.share.allow_download)}`}</code>
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
                <h2 className={`app-page-title ${styles.sectionTitle}`}>
                  {formatUiMessage(locale, "sharedPublic.sourcesSectionTitle")}
                </h2>
                <p className="app-page-subtitle">{formatUiMessage(locale, "sharedPublic.sourcesSectionSubtitle")}</p>
              </div>

              {payload.sources.length === 0 ? (
                <div className={styles.emptyState}>
                  <strong>{formatUiMessage(locale, "sharedPublic.sourcesEmptyTitle")}</strong>
                  <p className={styles.flushText}>{formatUiMessage(locale, "sharedPublic.sourcesEmptyBody")}</p>
                </div>
              ) : (
                <div className={styles.sourceList}>
                  {payload.sources.map((source) => (
                    <article className={styles.sourceCard} key={source.id}>
                      <div className={styles.sourceTitleRow}>
                        <strong>{source.file_name}</strong>
                        <code className={styles.semanticCode} data-status={normalizeSemanticValue(source.status)}>
                          {normalizeSemanticValue(source.status)}
                        </code>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>

            <section className={`app-surface-card ${styles.sectionStack}`}>
              <div>
                <h2 className={`app-page-title ${styles.sectionTitle}`}>
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
                <form className={styles.chatForm} onSubmit={handleSubmit}>
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
                  {TURNSTILE_SITE_KEY && !auth.token ? (
                    <div ref={turnstileRef} className={styles.turnstileHost} data-testid="share-turnstile" />
                  ) : null}
                  <div className="app-button-row">
                    <button
                      className="app-button-primary"
                      disabled={
                        answering ||
                        query.trim().length === 0 ||
                        (Boolean(TURNSTILE_SITE_KEY) && !auth.token && !turnstileToken.trim())
                      }
                      type="submit"
                    >
                      {answering ? formatUiMessage(locale, "sharedPublic.submitting") : formatUiMessage(locale, "sharedPublic.submitAction")}
                    </button>
                  </div>
                </form>
              ) : (
                <div className={`app-inline-surface ${styles.signInBox}`}>
                  <strong>{formatUiMessage(locale, "sharedPublic.signInRequiredTitle")}</strong>
                  <p className={styles.mutedText}>
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
                        <h3 className={styles.resultTitle}>{formatUiMessage(locale, "sharedPublic.answerTitle")}</h3>
                      </div>
                      <p className={styles.answerCopy}>{answerText}</p>
                    </section>
                  ) : null}

                  {citations.length > 0 ? (
                    <section className={styles.resultCard}>
                      <div className={styles.resultHeader}>
                        <h3 className={styles.resultTitle}>{formatUiMessage(locale, "sharedPublic.citationsTitle")}</h3>
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
                        <h3 className={styles.resultTitle}>{formatUiMessage(locale, "workspaceRightRail.sourcesSectionTitle")}</h3>
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
