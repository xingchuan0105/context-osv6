"use client";

import { useUiPreferences } from "../../../lib/ui-preferences";
import type { WorkspaceWebSourcesRequest } from "../../../lib/workspace/model";
import styles from "../workspace-right-rail.module.css";

export function WebSourcesTakeover({
  activeWebSources,
  onCloseWebSources,
}: {
  activeWebSources: WorkspaceWebSourcesRequest;
  onCloseWebSources?: () => void;
}) {
  const { locale } = useUiPreferences();

  return (
    <div className={`${styles.rail} ${styles.railTakeover}`}>
      <div className={styles.takeoverSection}>
        <div className={styles.webSourcesHeader}>
          <span className={styles.webSourcesCount}>
            {locale === "zh-CN"
              ? `${activeWebSources.sources.length} 个来源`
              : `${activeWebSources.sources.length} source${activeWebSources.sources.length > 1 ? "s" : ""}`}
          </span>
          <button
            aria-label={locale === "zh-CN" ? "关闭来源" : "Close sources"}
            className={styles.webSourcesClose}
            onClick={onCloseWebSources}
            type="button"
          >
            <svg
              aria-hidden="true"
              fill="none"
              height="20"
              stroke="currentColor"
              viewBox="0 0 24 24"
              width="20"
            >
              <path
                d="M18 6 6 18M6 6l12 12"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="1.8"
              />
            </svg>
          </button>
        </div>

        <div className={styles.webSourcesList}>
          {activeWebSources.sources.map((source, index) => (
            <div className={styles.webSourceCard} key={`${source.url}-${index}`}>
              <div className={styles.webSourceTitle}>
                <a
                  className={styles.webSourceLink}
                  href={source.url}
                  rel="noreferrer"
                  target="_blank"
                >
                  {source.title || source.url}
                </a>
              </div>
              <div className={styles.webSourceUrl}>{source.url}</div>
              {source.snippet ? (
                <div className={styles.webSourceSnippet}>{source.snippet}</div>
              ) : null}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
