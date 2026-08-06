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
  formatDayLabel,
  sumViews,
} from "../share/parts/share-center-utils";
import analyzeStyles from "../share/workspace-analyze-surface.module.css";

type WorkspaceAnalyticsRow = {
  workspace: DashboardWorkspace;
  analytics: ShareAnalyticsResponse | null;
  shareOn: boolean;
  error?: string;
};

/**
 * Dashboard-level analytics: aggregate share metrics across all workspaces
 * the owner has shared (or attempted to load analytics for).
 */
export function DashboardAnalyticsSurface() {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [rows, setRows] = useState<WorkspaceAnalyticsRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

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

  const totals = useMemo(() => {
    let views = 0;
    let visitors = 0;
    const byDay: Record<string, number> = {};
    for (const row of sharedRows) {
      const a = row.analytics;
      if (!a) continue;
      views += a.total_views;
      visitors += a.total_unique_visitors;
      for (const [day, n] of Object.entries(a.views_by_day)) {
        byDay[day] = (byDay[day] ?? 0) + n;
      }
    }
    return { views, visitors, byDay };
  }, [sharedRows]);

  const trendSeries = useMemo(
    () => buildDailyViewsSeries({ views_by_day: totals.byDay, total_views: totals.views, total_unique_visitors: totals.visitors }, 30),
    [totals],
  );
  const maxViews = Math.max(...trendSeries.map((e) => e.views), 1);
  const totalTrend = sumViews(trendSeries);

  return (
    <main className="app-page-shell" data-testid="dashboard-analytics-surface">
      <div className={`app-page-center ${analyzeStyles.pageStack}`}>
        <header className={analyzeStyles.header}>
          <Link className="app-link app-link-muted" href="/dashboard">
            {locale === "zh-CN" ? "← 工作台" : "← Dashboard"}
          </Link>
          <div className={analyzeStyles.headerRow}>
            <div>
              <h1 className="app-page-title">
                {locale === "zh-CN" ? "分享数据分析" : "Share analytics"}
              </h1>
              <p className="app-page-subtitle">
                {locale === "zh-CN"
                  ? "聚合你名下所有已开启分享的工作区访问趋势；可下钻到单个工作区。"
                  : "Aggregate views across all shared workspaces; drill into any one."}
              </p>
            </div>
          </div>
        </header>

        {error ? <p className="app-notice-banner">{error}</p> : null}

        {loading ? (
          <section className="app-surface-card">
            <p className={analyzeStyles.flushText}>
              {locale === "zh-CN" ? "加载中…" : "Loading…"}
            </p>
          </section>
        ) : (
          <>
            <section className={`app-surface-card ${analyzeStyles.sectionCard}`}>
              <h2 className={`app-page-title ${analyzeStyles.sectionTitle}`}>
                {locale === "zh-CN" ? "汇总" : "Totals"}
              </h2>
              <div className={analyzeStyles.metricGrid}>
                <div className="app-inline-surface">
                  <h3 className={analyzeStyles.metricTitle}>
                    {locale === "zh-CN" ? "已分享工作区" : "Shared workspaces"}
                  </h3>
                  <p className={analyzeStyles.metricValue}>{sharedRows.length}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={analyzeStyles.metricTitle}>
                    {locale === "zh-CN" ? "总访问" : "Total views"}
                  </h3>
                  <p className={analyzeStyles.metricValue}>{totals.views}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={analyzeStyles.metricTitle}>
                    {locale === "zh-CN" ? "独立访客（代理）" : "Unique visitors (proxy)"}
                  </h3>
                  <p className={analyzeStyles.metricValue}>{totals.visitors}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={analyzeStyles.metricTitle}>
                    {locale === "zh-CN" ? "近 30 日访问" : "Last 30d views"}
                  </h3>
                  <p className={analyzeStyles.metricValue}>{totalTrend}</p>
                </div>
              </div>
            </section>

            <section className={`app-surface-card ${analyzeStyles.sectionCard}`}>
              <h2 className={`app-page-title ${analyzeStyles.sectionTitle}`}>
                {locale === "zh-CN" ? "访问趋势" : "View trend"}
              </h2>
              <p className="app-page-subtitle">
                {locale === "zh-CN"
                  ? "全部分享工作区的日访问叠加（柱状）。"
                  : "Stacked daily views across shared workspaces."}
              </p>
              <div
                className={`app-inline-surface ${analyzeStyles.chartPanel}`}
                data-testid="dashboard-analytics-chart"
              >
                {trendSeries.some((e) => e.views > 0) ? (
                  trendSeries.map((entry) => (
                    <div className={analyzeStyles.chartRow} key={entry.day}>
                      <span>{formatDayLabel(locale, entry.day)}</span>
                      <div aria-hidden="true" className={analyzeStyles.chartTrack}>
                        <div
                          className={analyzeStyles.chartFill}
                          style={{
                            width: `${Math.max(
                              entry.views === 0 ? 0 : 8,
                              (entry.views / maxViews) * 100,
                            )}%`,
                          }}
                        />
                      </div>
                      <strong>{entry.views}</strong>
                    </div>
                  ))
                ) : (
                  <p className={analyzeStyles.mutedText}>
                    {locale === "zh-CN"
                      ? "暂无访问数据。开启分享并产生访问后这里会出现趋势图。"
                      : "No view data yet. Enable share and wait for traffic."}
                  </p>
                )}
              </div>
            </section>

            <section className={`app-surface-card ${analyzeStyles.sectionCard}`}>
              <h2 className={`app-page-title ${analyzeStyles.sectionTitle}`}>
                {locale === "zh-CN" ? "按工作区" : "By workspace"}
              </h2>
              <div className={analyzeStyles.metricGrid}>
                {rows.map((row) => (
                  <div className="app-inline-surface" key={row.workspace.workspace_id}>
                    <h3 className={analyzeStyles.metricTitle}>
                      {row.workspace.title || row.workspace.name || row.workspace.workspace_id}
                    </h3>
                    <p className={analyzeStyles.flushText}>
                      {row.shareOn
                        ? locale === "zh-CN"
                          ? `访问 ${row.analytics?.total_views ?? 0} · 访客 ${row.analytics?.total_unique_visitors ?? 0}`
                          : `Views ${row.analytics?.total_views ?? 0} · visitors ${row.analytics?.total_unique_visitors ?? 0}`
                        : locale === "zh-CN"
                          ? "未开启分享"
                          : "Share off"}
                    </p>
                    <Link
                      className="app-link"
                      href={`/dashboard/${row.workspace.workspace_id}/analyze`}
                    >
                      {locale === "zh-CN" ? "下钻分析" : "Drill down"}
                    </Link>
                  </div>
                ))}
              </div>
            </section>
          </>
        )}
      </div>
    </main>
  );
}
