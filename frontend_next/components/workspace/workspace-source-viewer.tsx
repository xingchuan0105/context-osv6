"use client";

import { useEffect, useRef } from "react";

import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";
import type { WorkspaceParsedPreviewItem } from "../../lib/workspace/client";
import type { WorkspaceSource } from "../../lib/workspace/model";
import type { Citation } from "../../lib/workspace/stream";
import styles from "./workspace-right-rail.module.css";

type WorkspaceSourceViewerProps = {
  activePreviewIndex: number | null;
  citation: Citation | null;
  error: string;
  hasMore: boolean;
  loading: boolean;
  loadingMore: boolean;
  parsedPreview: WorkspaceParsedPreviewItem[];
  rawContent: string;
  source: WorkspaceSource | null;
  summary: string;
  onClose: () => void;
  onLoadMore: () => void;
};

export function WorkspaceSourceViewer({
  activePreviewIndex,
  citation,
  error,
  hasMore,
  loading,
  loadingMore,
  parsedPreview,
  rawContent,
  source,
  summary,
  onClose,
  onLoadMore,
}: WorkspaceSourceViewerProps) {
  const { locale } = useUiPreferences();
  const previewRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (activePreviewIndex === null) {
      return;
    }

    const target = previewRef.current?.querySelector<HTMLElement>(`[data-preview-index="${activePreviewIndex}"]`);

    if (target && typeof target.scrollIntoView === "function") {
      target.scrollIntoView({ block: "center" });
    }
  }, [activePreviewIndex, parsedPreview.length, source?.id]);

  return (
    <section className={styles.viewer} aria-label={formatUiMessage(locale, "workspaceRightRail.viewerSectionLabel")}>
      <div className={styles.viewerHeader}>
        <div className={styles.paneHeading}>
          <h3 className={styles.paneTitle}>
            {source?.file_name ?? formatUiMessage(locale, "workspaceRightRail.viewerSectionTitle")}
          </h3>
          <p className={styles.paneSubtitle}>
            {source?.title || formatUiMessage(locale, "workspaceRightRail.viewerSectionSubtitle")}
            {citation?.page !== null && citation?.page !== undefined
              ? ` - ${formatUiMessage(locale, "workspaceRightRail.viewerPage", {
                  page: String(citation.page),
                })}`
              : ""}
          </p>
        </div>
        <button className={styles.closeButton} onClick={onClose} type="button">
          {formatUiMessage(locale, "workspaceRightRail.closeViewerAction")}
        </button>
      </div>

      {citation ? (
        <div className={styles.viewerCitationCard}>
          <div className={styles.viewerCitationHeader}>
            <strong className={styles.viewerCitationTitle}>
              {citation.doc_name || source?.file_name || formatUiMessage(locale, "workspaceRightRail.citationFallbackTitle")}
            </strong>
            <span className={styles.statusBadge}>
              {formatUiMessage(locale, "workspaceRightRail.viewerScore", {
                score: citation.score.toFixed(2),
              })}
            </span>
          </div>
          {citation.preview ? <p className={styles.viewerCitationText}>{citation.preview}</p> : null}
          {citation.image_url ? (
            <img alt={citation.caption ?? citation.doc_name} className={styles.viewerCitationImage} src={citation.image_url} />
          ) : null}
          {citation.caption ? <p className={styles.viewerCaption}>{citation.caption}</p> : null}
          {citation.content ? <p className={styles.viewerCitationText}>{citation.content}</p> : null}
        </div>
      ) : null}

      {summary ? <div className={styles.viewerSummary}>{summary}</div> : null}
      {error ? <div className={styles.error}>{error}</div> : null}

      {loading ? (
        <div className={styles.emptyState}>{formatUiMessage(locale, "workspaceRightRail.loadingSourcePreview")}</div>
      ) : parsedPreview.length > 0 ? (
        <div className={styles.viewerPreview} ref={previewRef}>
          {parsedPreview.map((item, index) => (
            <article
              className={`${styles.viewerPreviewItem}${activePreviewIndex === index ? ` ${styles.viewerPreviewItemActive}` : ""}`}
              data-preview-index={index}
              key={`${item.page}-${item.cursor}-${index}`}
            >
              <div className={styles.viewerPreviewMeta}>
                {formatUiMessage(locale, "workspaceRightRail.viewerLocation", {
                  page: String(item.page),
                  cursor: String(item.cursor),
                })}
              </div>
              <div>{item.text}</div>
            </article>
          ))}

          {hasMore ? (
            <div className={styles.buttonRow}>
              <button className={styles.button} disabled={loadingMore} onClick={onLoadMore} type="button">
                {loadingMore
                  ? formatUiMessage(locale, "workspaceRightRail.loading")
                  : formatUiMessage(locale, "workspaceRightRail.viewerLoadMoreAction")}
              </button>
            </div>
          ) : null}
        </div>
      ) : rawContent ? (
        <pre className={styles.viewerRawContent}>{rawContent}</pre>
      ) : (
        <div className={styles.emptyStateBlock}>
          <div className={styles.emptyStateTitle}>{formatUiMessage(locale, "workspaceRightRail.viewerEmptyTitle")}</div>
          <div className={styles.emptyState}>{formatUiMessage(locale, "workspaceRightRail.viewerEmptyBody")}</div>
        </div>
      )}
    </section>
  );
}
