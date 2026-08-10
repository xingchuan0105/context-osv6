"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { useUiPreferences } from "../../../lib/ui-preferences";
import type { WorkspaceSource } from "../../../lib/workspace/model";
import styles from "./share-tabs.module.css";

export type ShareSourcesTabProps = {
  sources: WorkspaceSource[];
  onOpenSource: (sourceId: string) => void;
};

export function isSourceOpenable(status: string | null | undefined) {
  const normalized = status?.trim().toLowerCase();
  return normalized === "ready" || normalized === "completed";
}

/** Localized source status label (reuses workspaceRightRail.sourceStatus.* keys). */
export function sourceStatusLabel(locale: "zh-CN" | "en", status: string) {
  // Switch on a const narrows to literals so the template key typechecks (UiMessageKey).
  const normalized = status.trim().toLowerCase();
  switch (normalized) {
    case "pending":
    case "enqueueing":
    case "queued":
    case "processing":
    case "indexing":
    case "completed":
    case "ready":
    case "failed":
    case "error":
      return formatUiMessage(locale, `workspaceRightRail.sourceStatus.${normalized}`);
    default:
      return status;
  }
}

function extBadge(fileName: string) {
  const ext = fileName.split(".").pop()?.trim().toUpperCase() ?? "";
  return ext && ext !== fileName.toUpperCase() ? ext.slice(0, 4) : "DOC";
}

/**
 * Single-column source list (X main-column width). Ready sources are
 * clickable and open the detail modal; processing/failed ones stay disabled.
 */
export function ShareSourcesTab({ sources, onOpenSource }: ShareSourcesTabProps) {
  const { locale } = useUiPreferences();

  return (
    <section className={styles.tabPane} data-testid="shared-sources-tab">
      {sources.length === 0 ? (
        <p className={styles.emptyState}>
          {formatUiMessage(locale, "sharedPublic.sourcesTabEmpty")}
        </p>
      ) : (
        <ul className={styles.sourceCards}>
          {sources.map((source) => {
            const openable = isSourceOpenable(source.status);
            const normalized = source.status.trim().toLowerCase() || "unknown";
            return (
              <li key={source.id}>
                <button
                  type="button"
                  className={`${styles.sourceCard} ${openable ? "" : styles.sourceCardDisabled}`}
                  disabled={!openable}
                  data-testid={`share-source-card-${source.id}`}
                  onClick={() => onOpenSource(source.id)}
                >
                  <span className={styles.sourceExt} aria-hidden>
                    {extBadge(source.file_name)}
                  </span>
                  <span className={styles.sourceName} title={source.file_name}>
                    {source.file_name}
                  </span>
                  <span className={styles.sourceStatus} data-status={normalized}>
                    {sourceStatusLabel(locale, source.status)}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
