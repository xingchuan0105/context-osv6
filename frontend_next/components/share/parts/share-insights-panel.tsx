"use client";

import { formatUiMessage } from "../../../lib/i18n/messages";
import { InsightMetricCard, SectionHeader } from "./share-center-ui";
import styles from "./share-insights-panel.module.css";
import { formatDayLabel } from "./share-center-utils";
import type { useShareCenter } from "./use-share-center";

type ShareCenter = ReturnType<typeof useShareCenter>;

export function ShareInsightsPanel({ center }: { center: ShareCenter }) {
  const {
    accessLogsQuery,
    activeDaysValue,
    analyticsQuery,
    latestAccessValue,
    locale,
    recentViewsValue,
    setTrendWindowDays,
    shareStatusText,
    totalViewsValue,
    trendSeries,
    trendWindowDays,
  } = center;

  return (
    <>
      <section
          className={`app-surface-card ${styles.overviewSection}`}
          id="insights"
        >
          <SectionHeader
            subtitle={formatUiMessage(locale, "shareCenter.overviewSectionSubtitle")}
            title={formatUiMessage(locale, "shareCenter.overviewSectionTitle")}
          />

          {analyticsQuery.error && !analyticsQuery.data ? (
            <p className="app-notice-banner">
              {analyticsQuery.error instanceof Error
                ? analyticsQuery.error.message
                : formatUiMessage(locale, "shareCenter.analyticsLoadError")}
            </p>
          ) : null}

          {accessLogsQuery.error && !accessLogsQuery.data ? (
            <p className="app-notice-banner">
              {accessLogsQuery.error instanceof Error
                ? accessLogsQuery.error.message
                : formatUiMessage(locale, "shareCenter.accessLogsLoadError")}
            </p>
          ) : null}

          <div className={styles.metricGrid}>
            <InsightMetricCard
              title={formatUiMessage(locale, "shareCenter.overviewCurrentStatus")}
              value={shareStatusText}
            />
            <InsightMetricCard
              title={formatUiMessage(locale, "shareCenter.overviewTotalViews")}
              value={totalViewsValue}
            />
            <InsightMetricCard
              title={formatUiMessage(locale, "shareCenter.overviewRecentViews")}
              value={recentViewsValue}
            />
            <InsightMetricCard
              title={formatUiMessage(locale, "shareCenter.overviewActiveDays")}
              value={activeDaysValue}
            />
            <InsightMetricCard
              title={formatUiMessage(locale, "shareCenter.overviewLastAccess")}
              value={latestAccessValue}
            />
          </div>
        </section>

        {analyticsQuery.isLoading && !analyticsQuery.data ? (
          <section className="app-surface-card">
            <p className={styles.flushText}>
              {formatUiMessage(locale, "shareCenter.analyticsLoading")}
            </p>
          </section>
        ) : analyticsQuery.error && !analyticsQuery.data ? (
          <section className="app-surface-card">
            <p className="app-notice-banner">
              {analyticsQuery.error instanceof Error
                ? analyticsQuery.error.message
                : formatUiMessage(locale, "shareCenter.analyticsLoadError")}
            </p>
          </section>
        ) : (
          <section
            className={`app-surface-card ${styles.trendSection}`}
          >
            <div className={styles.trendHeader}>
              <div className={styles.trendHeaderTitle}>
                <SectionHeader
                  subtitle={formatUiMessage(locale, "shareCenter.trendSectionSubtitle")}
                  title={formatUiMessage(locale, "shareCenter.trendSectionTitle")}
                />
              </div>
              <div className={`app-button-row ${styles.trendRangeRow}`}>
                <button
                  className={`${trendWindowDays === 7 ? "app-button-secondary" : "app-button-ghost"} ${styles.trendRangeButton}`}
                  type="button"
                  onClick={() => setTrendWindowDays(7)}
                >
                  {formatUiMessage(locale, "shareCenter.trendRange7")}
                </button>
                <button
                  className={`${trendWindowDays === 30 ? "app-button-secondary" : "app-button-ghost"} ${styles.trendRangeButton}`}
                  type="button"
                  onClick={() => setTrendWindowDays(30)}
                >
                  {formatUiMessage(locale, "shareCenter.trendRange30")}
                </button>
              </div>
            </div>

            {trendSeries.some((entry) => entry.views > 0) ? (
              <div
                className={`app-inline-surface ${styles.chartPanel}`}
                data-testid="analyze-chart"
              >
                {trendSeries.map((entry) => (
                  <div
                    className={styles.chartRow}
                    key={entry.day}
                  >
                    <span>{formatDayLabel(locale, entry.day)}</span>
                    <div
                      aria-hidden="true"
                      className={styles.chartTrack}
                    >
                      <div
                        className={styles.chartFill}
                        style={{
                          width: `${Math.max(
                            entry.views === 0 ? 0 : 8,
                            (entry.views /
                              Math.max(...trendSeries.map((seriesEntry) => seriesEntry.views), 1)) *
                              100,
                          )}%`,
                        }}
                      />
                    </div>
                    <strong>{entry.views}</strong>
                  </div>
                ))}
              </div>
            ) : (
              <div className={`app-inline-surface ${styles.emptyPanel}`}>
                <strong>{formatUiMessage(locale, "shareCenter.trendEmptyTitle")}</strong>
                <p className={styles.mutedText}>
                  {formatUiMessage(locale, "shareCenter.trendEmptyBody")}
                </p>
              </div>
            )}
          </section>
        )}
    </>
  );
}
