"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { listWorkspaces, type DashboardWorkspace } from "../../lib/dashboard/client";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  getShareAnalytics,
  getShareSettings,
  isShareEnabled,
  type ShareAnalyticsResponse,
} from "../../lib/share/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  buildDailyViewsSeries,
  sumViews,
} from "../share/parts/share-center-utils";
import { ShareViewsBarChart } from "../share/parts/share-views-bar-chart";
import styles from "./dashboard-analytics-surface.module.css";

type WorkspaceAnalyticsRow = {
  workspace: DashboardWorkspace;
  analytics: ShareAnalyticsResponse | null;
  shareOn: boolean;
  error?: string;
};

type RangeDays = 7 | 30;

/**
 * Dashboard share analytics — product language + DeepSeek-style usage layout:
 * summary metric cards, range filter, primary chart, per-workspace panels.
 */
export function DashboardAnalyticsSurface() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [rows, setRows] = useState<WorkspaceAnalyticsRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [rangeDays, setRangeDays] = useState<RangeDays>(30);
  const [filterWs, setFilterWs] = useState<string>("all");

  useEffect(() => {
    let cancelled = false;

    async function load() {
      if (!auth.token) {
        setLoading(false);
        return;
      }
      setLoading(true);
      setError("");
      try {
        const list = await listWorkspaces(auth.token);
        const results = await Promise.all(
          list.workspaces.map(async (workspace) => {
            const workspaceId = workspace.workspace_id;
            try {
              const settings = await getShareSettings(auth.token as string, workspaceId);
              const shareOn = isShareEnabled(settings);
              if (!shareOn) {
                return {
                  workspace,
                  analytics: null,
                  shareOn: false,
                } satisfies WorkspaceAnalyticsRow;
              }
              const analytics = await getShareAnalytics(auth.token as string, workspaceId);
              return { workspace, analytics, shareOn: true } satisfies WorkspaceAnalyticsRow;
            } catch (err) {
              return {
                workspace,
                analytics: null,
                shareOn: false,
                error: err instanceof Error ? err.message : "load failed",
              } satisfies WorkspaceAnalyticsRow;
            }
          }),
        );
        if (!cancelled) {
          setRows(results);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load workspaces");
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [auth.token, auth.user?.id]);

  const sharedRows = useMemo(() => rows.filter((r) => r.shareOn && r.analytics), [rows]);

  const visibleShared = useMemo(() => {
    if (filterWs === "all") {
      return sharedRows;
    }
    return sharedRows.filter((r) => r.workspace.workspace_id === filterWs);
  }, [filterWs, sharedRows]);

  const totals = useMemo(() => {
    let views = 0;
    let visitors = 0;
    const byDay: Record<string, number> = {};
    for (const row of visibleShared) {
      const a = row.analytics;
      if (!a) continue;
      views += a.total_views;
      visitors += a.total_unique_visitors;
      for (const [day, n] of Object.entries(a.views_by_day)) {
        byDay[day] = (byDay[day] ?? 0) + n;
      }
    }
    return { views, visitors, byDay };
  }, [visibleShared]);

  const trendSeries = useMemo(
    () =>
      buildDailyViewsSeries(
        {
          views_by_day: totals.byDay,
          total_views: totals.views,
          total_unique_visitors: totals.visitors,
        },
        rangeDays,
      ),
    [totals, rangeDays],
  );
  const rangeViews = sumViews(trendSeries);

  return (
    <main className={styles.page} data-testid="dashboard-analytics-surface">
      <div className={styles.inner}>
        <header className={styles.header}>
          <Link className="app-link app-link-muted" href="/dashboard">
            {formatUiMessage(locale, "dashboardBackToDashboard")}
          </Link>
          <div className={styles.headerRow}>
            <div>
              <h1 className={styles.title}>
                {formatUiMessage(locale, "dashboardShareAnalyticsTitle")}
              </h1>
              <p className={styles.subtitle}>
                {formatUiMessage(locale, "dashboardShareAnalyticsSubtitle")}
              </p>
            </div>
          </div>
        </header>

        {error ? <p className="app-notice-banner">{error}</p> : null}

        {loading ? (
          <section className={styles.card}>
            <p className={styles.muted}>{formatUiMessage(locale, "dashboardLoading")}</p>
          </section>
        ) : (
          <>
            {/* Filters first — DeepSeek 时间维度 / 维度筛选 */}
            <div className={styles.toolbar}>
              <div className={styles.chip} role="group" aria-label="range">
                <span className={styles.chipLabel}>
                  {formatUiMessage(locale, "dashboardTimeDimension")}
                </span>
                <div className={styles.pills}>
                  <button
                    type="button"
                    className={rangeDays === 7 ? styles.pillActive : styles.pill}
                    onClick={() => setRangeDays(7)}
                  >
                    {formatUiMessage(locale, "dashboardRange7")}
                  </button>
                  <button
                    type="button"
                    className={rangeDays === 30 ? styles.pillActive : styles.pill}
                    onClick={() => setRangeDays(30)}
                  >
                    {formatUiMessage(locale, "dashboardRange30")}
                  </button>
                </div>
              </div>
              <label className={styles.chip}>
                <span className={styles.chipLabel}>
                  {formatUiMessage(locale, "dashboardFilterWorkspace")}
                </span>
                <select
                  className={styles.chipSelect}
                  value={filterWs}
                  onChange={(e) => setFilterWs(e.target.value)}
                  data-testid="analytics-workspace-filter"
                >
                  <option value="all">{formatUiMessage(locale, "dashboardFilterAll")}</option>
                  {sharedRows.map((r) => (
                    <option key={r.workspace.workspace_id} value={r.workspace.workspace_id}>
                      {r.workspace.title || r.workspace.name || r.workspace.workspace_id}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div className={styles.metricRow}>
              <div className={styles.metricCard}>
                <span className={styles.metricLabel}>
                  {formatUiMessage(locale, "dashboardSharedWorkspaces")}
                </span>
                <strong className={styles.metricValue}>{sharedRows.length}</strong>
              </div>
              <div className={styles.metricCard}>
                <span className={styles.metricLabel}>
                  {formatUiMessage(locale, "dashboardTotalViews")}
                </span>
                <strong className={styles.metricValue}>{totals.views.toLocaleString()}</strong>
              </div>
              <div className={styles.metricCard}>
                <span className={styles.metricLabel}>
                  {formatUiMessage(locale, "dashboardUniqueVisitors")}
                </span>
                <strong className={styles.metricValue}>{totals.visitors.toLocaleString()}</strong>
              </div>
              <div className={styles.metricCard}>
                <span className={styles.metricLabel}>
                  {rangeDays === 7
                    ? formatUiMessage(locale, "dashboardViews7d")
                    : formatUiMessage(locale, "dashboardViews30d")}
                </span>
                <strong className={styles.metricValue}>{rangeViews.toLocaleString()}</strong>
              </div>
            </div>

            <section className={styles.chartCard} data-testid="dashboard-analytics-chart">
              <div className={styles.chartHead}>
                <div>
                  <h2 className={styles.chartTitle}>
                    {formatUiMessage(locale, "dashboardViewTrend")}
                  </h2>
                  <p className={styles.chartSub}>
                    {formatUiMessage(locale, "dashboardViewTrendSubtitle")}
                  </p>
                </div>
                <strong className={styles.chartTotal}>
                  {formatUiMessage(locale, "dashboardChartTotal", {
                    n: rangeViews.toLocaleString(),
                  })}
                </strong>
              </div>
              <ShareViewsBarChart
                series={trendSeries}
                locale={locale}
                height={280}
                emptyLabel={formatUiMessage(locale, "dashboardTrendEmpty")}
              />
            </section>

            <section className={styles.breakdown}>
              <h2 className={styles.breakdownTitle}>
                {formatUiMessage(locale, "dashboardByWorkspace")}
              </h2>
              <div className={styles.wsGrid}>
                {rows.map((row) => {
                  const series = row.analytics
                    ? buildDailyViewsSeries(row.analytics, rangeDays)
                    : [];
                  const views = row.analytics?.total_views ?? 0;
                  const visitors = row.analytics?.total_unique_visitors ?? 0;
                  const name =
                    row.workspace.title || row.workspace.name || row.workspace.workspace_id;
                  return (
                    <article className={styles.wsCard} key={row.workspace.workspace_id}>
                      <div className={styles.wsCardHead}>
                        <h3 className={styles.wsName}>{name}</h3>
                        <Link
                          className="app-link app-link-muted"
                          href={`/dashboard/${row.workspace.workspace_id}/analyze`}
                        >
                          {formatUiMessage(locale, "dashboardDrillDown")}
                        </Link>
                      </div>
                      {/* Inline totals like DeepSeek "API 请求次数 35,881" */}
                      <p className={styles.wsMetricInline}>
                        {formatUiMessage(locale, "dashboardTotalViews")}
                        <strong>{views.toLocaleString()}</strong>
                        <span aria-hidden="true"> · </span>
                        {formatUiMessage(locale, "dashboardUniqueVisitors")}
                        <strong>{visitors.toLocaleString()}</strong>
                      </p>
                      {row.shareOn ? (
                        <ShareViewsBarChart
                          series={series}
                          locale={locale}
                          height={140}
                          emptyLabel={formatUiMessage(locale, "dashboardTrendEmpty")}
                        />
                      ) : (
                        <p className={styles.muted}>
                          {formatUiMessage(locale, "dashboardShareOff")}
                        </p>
                      )}
                    </article>
                  );
                })}
              </div>
            </section>
          </>
        )}
      </div>
    </main>
  );
}
