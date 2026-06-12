"use client";

import { formatSettingsShareMessage } from "../../../lib/settings-share-messages";
import { InsightMetricCard, SectionHeader } from "./share-center-ui";
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
          className="app-surface-card"
          id="insights"
          style={{
            background:
              "linear-gradient(180deg, hsl(var(--background)) 0%, hsl(var(--muted) / 0.42) 100%)",
            display: "grid",
            gap: "0.95rem",
            padding: "0.95rem 1rem 1rem",
            scrollMarginTop: "6rem",
          }}
        >
          <SectionHeader
            subtitle={formatSettingsShareMessage(locale, "shareCenter.overviewSectionSubtitle")}
            title={formatSettingsShareMessage(locale, "shareCenter.overviewSectionTitle")}
          />

          {analyticsQuery.error && !analyticsQuery.data ? (
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {analyticsQuery.error instanceof Error
                ? analyticsQuery.error.message
                : formatSettingsShareMessage(locale, "shareCenter.analyticsLoadError")}
            </p>
          ) : null}

          {accessLogsQuery.error && !accessLogsQuery.data ? (
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {accessLogsQuery.error instanceof Error
                ? accessLogsQuery.error.message
                : formatSettingsShareMessage(locale, "shareCenter.accessLogsLoadError")}
            </p>
          ) : null}

          <div
            style={{
              display: "grid",
              gap: "1rem",
              gridTemplateColumns: "minmax(0, 1fr)",
            }}
          >
            <InsightMetricCard
              title={formatSettingsShareMessage(locale, "shareCenter.overviewCurrentStatus")}
              value={shareStatusText}
            />
            <InsightMetricCard
              title={formatSettingsShareMessage(locale, "shareCenter.overviewTotalViews")}
              value={totalViewsValue}
            />
            <InsightMetricCard
              title={formatSettingsShareMessage(locale, "shareCenter.overviewRecentViews")}
              value={recentViewsValue}
            />
            <InsightMetricCard
              title={formatSettingsShareMessage(locale, "shareCenter.overviewActiveDays")}
              value={activeDaysValue}
            />
            <InsightMetricCard
              title={formatSettingsShareMessage(locale, "shareCenter.overviewLastAccess")}
              value={latestAccessValue}
            />
          </div>
        </section>

        {analyticsQuery.isLoading && !analyticsQuery.data ? (
          <section className="app-surface-card">
            <p style={{ margin: 0 }}>
              {formatSettingsShareMessage(locale, "shareCenter.analyticsLoading")}
            </p>
          </section>
        ) : analyticsQuery.error && !analyticsQuery.data ? (
          <section className="app-surface-card">
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {analyticsQuery.error instanceof Error
                ? analyticsQuery.error.message
                : formatSettingsShareMessage(locale, "shareCenter.analyticsLoadError")}
            </p>
          </section>
        ) : (
          <section
            className="app-surface-card"
            style={{
              background:
                "linear-gradient(180deg, hsl(var(--background)) 0%, hsl(var(--muted) / 0.28) 100%)",
              display: "grid",
              gap: "0.95rem",
              padding: "0.95rem 1rem 1rem",
            }}
          >
            <div
              style={{
                display: "grid",
                gap: "1rem",
              }}
            >
              <div style={{ minWidth: 0 }}>
                <SectionHeader
                  subtitle={formatSettingsShareMessage(locale, "shareCenter.trendSectionSubtitle")}
                  title={formatSettingsShareMessage(locale, "shareCenter.trendSectionTitle")}
                />
              </div>
              <div className="app-button-row" style={{ justifyContent: "flex-start" }}>
                <button
                  className={trendWindowDays === 7 ? "app-button-secondary" : "app-button-ghost"}
                  style={{
                    fontSize: "0.84rem",
                    minHeight: "2.18rem",
                    padding: "0.48rem 0.72rem",
                  }}
                  type="button"
                  onClick={() => setTrendWindowDays(7)}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.trendRange7")}
                </button>
                <button
                  className={trendWindowDays === 30 ? "app-button-secondary" : "app-button-ghost"}
                  style={{
                    fontSize: "0.84rem",
                    minHeight: "2.18rem",
                    padding: "0.48rem 0.72rem",
                  }}
                  type="button"
                  onClick={() => setTrendWindowDays(30)}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.trendRange30")}
                </button>
              </div>
            </div>

            {trendSeries.some((entry) => entry.views > 0) ? (
              <div
                className="app-inline-surface"
                style={{
                  display: "grid",
                  gap: "0.52rem",
                  padding: "0.82rem 0.9rem 0.88rem",
                }}
              >
                {trendSeries.map((entry) => (
                  <div
                    key={entry.day}
                    style={{
                      alignItems: "center",
                      display: "grid",
                      gap: "0.6rem",
                      gridTemplateColumns: "4.2rem 1fr auto",
                    }}
                  >
                    <span>{formatDayLabel(locale, entry.day)}</span>
                    <div
                      aria-hidden="true"
                      style={{
                        background: "hsl(var(--muted))",
                        borderRadius: "999px",
                        height: "0.65rem",
                        overflow: "hidden",
                      }}
                    >
                      <div
                        style={{
                          background: "hsl(var(--primary))",
                          borderRadius: "999px",
                          height: "100%",
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
              <div className="app-inline-surface" style={{ display: "grid", gap: "0.25rem" }}>
                <strong>{formatSettingsShareMessage(locale, "shareCenter.trendEmptyTitle")}</strong>
                <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
                  {formatSettingsShareMessage(locale, "shareCenter.trendEmptyBody")}
                </p>
              </div>
            )}
          </section>
        )}
    </>
  );
}
