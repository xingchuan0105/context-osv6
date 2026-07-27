"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { SectionHeader } from "./share-center-ui";
import styles from "./share-activity-panel.module.css";
import { formatAccessedAt, parseAccessedAt } from "./share-center-utils";
import type { useShareCenter } from "./use-share-center";

type ShareCenter = ReturnType<typeof useShareCenter>;

export function ShareActivityPanel({ center }: { center: ShareCenter }) {
  const { accessLogsQuery, locale } = center;

  return (
    <>
      {accessLogsQuery.isLoading && !accessLogsQuery.data ? (
          <section className={`app-surface-card ${styles.sectionAnchor}`} id="activity">
            <p className={styles.flushText}>
              {formatUiMessage(locale, "shareCenter.accessLogsLoading")}
            </p>
          </section>
        ) : accessLogsQuery.error && !accessLogsQuery.data ? (
          <section className={`app-surface-card ${styles.sectionAnchor}`} id="activity">
            <p className="app-notice-banner">
              {accessLogsQuery.error instanceof Error
                ? accessLogsQuery.error.message
                : formatUiMessage(locale, "shareCenter.accessLogsLoadError")}
            </p>
          </section>
        ) : (
          <section
            className={`app-surface-card ${styles.activitySection}`}
            id="activity"
          >
            <SectionHeader
              subtitle={formatUiMessage(locale, "shareCenter.activitySectionSubtitle")}
              title={formatUiMessage(locale, "shareCenter.activitySectionTitle")}
            />

            {accessLogsQuery.data && accessLogsQuery.data.logs.length > 0 ? (
              <div className={styles.logList}>
                {accessLogsQuery.data.logs
                  .slice()
                  .sort((left, right) => {
                    const leftTime = parseAccessedAt(left.accessed_at) ?? 0;
                    const rightTime = parseAccessedAt(right.accessed_at) ?? 0;
                    return rightTime - leftTime;
                  })
                  .slice(0, 10)
                  .map((log) => (
                    <div
                      className={`app-inline-surface ${styles.logItem}`}
                      key={log.id}
                    >
                      <div className={styles.logField}>
                        <span className={styles.logLabel}>
                          {formatUiMessage(locale, "shareCenter.activityActionLabel")}
                        </span>
                        <strong>{log.action}</strong>
                      </div>
                      <div className={styles.logField}>
                        <span className={styles.logLabel}>
                          {formatUiMessage(locale, "shareCenter.activityTimeLabel")}
                        </span>
                        <span>{formatAccessedAt(locale, log.accessed_at)}</span>
                      </div>
                    </div>
                  ))}
              </div>
            ) : (
              <div className={`app-inline-surface ${styles.emptyPanel}`}>
                <strong>{formatUiMessage(locale, "shareCenter.activityEmptyTitle")}</strong>
                <p className={styles.mutedText}>
                  {formatUiMessage(locale, "shareCenter.activityEmptyBody")}
                </p>
              </div>
            )}
          </section>
        )}
    </>
  );
}
