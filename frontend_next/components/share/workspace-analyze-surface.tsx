"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  getShareAccessLogs,
  getShareAnalytics,
  getShareSettings,
  isShareEnabled,
  type AccessLogsResponse,
  type ShareAnalyticsResponse,
  type ShareSettings,
} from "../../lib/share/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  buildDailyViewsSeries,
  countActiveDays,
  sumViews,
} from "./parts/share-center-utils";
import { ShareViewsBarChart } from "./parts/share-views-bar-chart";
import styles from "./workspace-analyze-surface.module.css";

function AnalyzeSection({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <section className={`app-surface-card ${styles.sectionCard}`}>
      <div>
        <h2 className={`app-page-title ${styles.sectionTitle}`}>{title}</h2>
        <p className="app-page-subtitle">{subtitle}</p>
      </div>
      {children}
    </section>
  );
}

export function WorkspaceAnalyzeSurface({ workspaceId }: { workspaceId: string }) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const [settings, setSettings] = useState<ShareSettings | null>(null);
  const [analytics, setAnalytics] = useState<ShareAnalyticsResponse | null>(null);
  const [logs, setLogs] = useState<AccessLogsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;

    async function loadAnalyzeData() {
      if (!auth.token) {
        setLoading(false);
        return;
      }

      setLoading(true);
      setError("");

      const [settingsResult, analyticsResult, logsResult] = await Promise.allSettled([
        getShareSettings(auth.token, workspaceId),
        getShareAnalytics(auth.token, workspaceId),
        getShareAccessLogs(auth.token, workspaceId),
      ]);

      if (cancelled) {
        return;
      }

      if (settingsResult.status === "fulfilled") {
        setSettings(settingsResult.value);
      } else {
        setError(formatUiMessage(locale, "shareAnalyze.loadSettingsError"));
      }

      if (analyticsResult.status === "fulfilled") {
        setAnalytics(analyticsResult.value);
      } else {
        setError((current) => current || formatUiMessage(locale, "shareAnalyze.loadAnalyticsError"));
      }

      if (logsResult.status === "fulfilled") {
        setLogs(logsResult.value);
      } else {
        setError((current) => current || formatUiMessage(locale, "shareAnalyze.loadLogsError"));
      }

      setLoading(false);
    }

    void loadAnalyzeData();

    return () => {
      cancelled = true;
    };
  }, [auth.token, locale, workspaceId]);

  const trendSeries = useMemo(() => buildDailyViewsSeries(analytics, 30), [analytics]);
  const activeDays = useMemo(() => countActiveDays(trendSeries), [trendSeries]);
  const accessActions = useMemo(() => sumViews(trendSeries), [trendSeries]);

  return (
    <main className="app-page-shell">
      <div className={`app-page-center ${styles.pageStack}`}>
        <header className={styles.header}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceId}`}>
            {formatUiMessage(locale, "shareAnalyze.backWorkspace")}
          </Link>
          <div className={styles.headerRow}>
            <div>
              <h1 className="app-page-title">{formatUiMessage(locale, "shareAnalyze.title")}</h1>
              <p className="app-page-subtitle">
                {formatUiMessage(locale, "shareAnalyze.subtitle")}
              </p>
            </div>
            <Link className="app-button-secondary" href={`/dashboard/${workspaceId}/share`}>
              {formatUiMessage(locale, "shareAnalyze.goShare")}
            </Link>
          </div>
        </header>

        {error ? <p className="app-notice-banner">{error}</p> : null}

        {loading ? (
          <section className="app-surface-card">
            <p className={styles.flushText}>{formatUiMessage(locale, "shareAnalyze.loading")}</p>
          </section>
        ) : !isShareEnabled(settings) ? (
          <AnalyzeSection
            subtitle={formatUiMessage(locale, "shareAnalyze.emptySubtitle")}
            title={formatUiMessage(locale, "shareAnalyze.emptyTitle")}
          >
            <div className="app-button-row">
              <Link className="app-button-primary" href={`/dashboard/${workspaceId}/share`}>
                {formatUiMessage(locale, "shareAnalyze.goShare")}
              </Link>
            </div>
          </AnalyzeSection>
        ) : (
          <>
            <AnalyzeSection
              subtitle={formatUiMessage(locale, "shareAnalyze.statusSubtitle")}
              title={formatUiMessage(locale, "shareAnalyze.statusTitle")}
            >
              <div className={styles.metricGrid}>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.accessLevel")}
                  </h3>
                  <p className={styles.flushText}>
                    {settings?.access_level ?? formatUiMessage(locale, "shareAnalyze.notSet")}
                  </p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.allowDownload")}
                  </h3>
                  <p className={styles.flushText}>
                    {settings?.allow_download
                      ? formatUiMessage(locale, "shareAnalyze.on")
                      : formatUiMessage(locale, "shareAnalyze.off")}
                  </p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.expiresAt")}
                  </h3>
                  <p className={styles.flushText}>
                    {settings?.expires_at ?? formatUiMessage(locale, "shareAnalyze.notSet")}
                  </p>
                </div>
              </div>
            </AnalyzeSection>

            <AnalyzeSection
              subtitle={formatUiMessage(locale, "shareAnalyze.metricsSubtitle")}
              title={formatUiMessage(locale, "shareAnalyze.metricsTitle")}
            >
              <div className={styles.metricGrid}>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.totalViews")}
                  </h3>
                  <p className={styles.metricValue}>{analytics?.total_views ?? 0}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.uniqueVisitors")}
                  </h3>
                  <p className={styles.metricValue}>{analytics?.total_unique_visitors ?? 0}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.activeDays")}
                  </h3>
                  <p className={styles.metricValue}>{activeDays}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>
                    {formatUiMessage(locale, "shareAnalyze.questionsProxy")}
                  </h3>
                  <p className={styles.metricValue}>{accessActions}</p>
                </div>
              </div>
            </AnalyzeSection>

            <AnalyzeSection
              subtitle={formatUiMessage(locale, "shareAnalyze.trendSubtitle")}
              title={formatUiMessage(locale, "shareAnalyze.trendTitle")}
            >
              <div className={`app-inline-surface ${styles.chartPanel}`} data-testid="analyze-chart">
                <ShareViewsBarChart
                  series={trendSeries}
                  locale={locale}
                  emptyLabel={formatUiMessage(locale, "shareAnalyze.trendEmpty")}
                />
              </div>
            </AnalyzeSection>

            <AnalyzeSection
              subtitle={formatUiMessage(locale, "shareAnalyze.logsSubtitle")}
              title={formatUiMessage(locale, "shareAnalyze.logsTitle")}
            >
              <p className={`app-form-footnote ${styles.mutedText}`}>
                {formatUiMessage(locale, "shareAnalyze.visitorPrivacyNote")}
              </p>
              {logs?.logs.length ? (
                <ul className={styles.logList}>
                  {logs.logs.slice(0, 10).map((log) => (
                    <li className="app-inline-surface" key={log.id}>
                      <strong>{log.action}</strong>
                      <div className={styles.logMeta}>
                        <span title={log.visitor_id}>
                          {log.visitor_id.length > 16
                            ? `${log.visitor_id.slice(0, 8)}…${log.visitor_id.slice(-4)}`
                            : log.visitor_id}
                        </span>
                        {" · "}
                        {log.accessed_at}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : (
                <div className="app-inline-surface">
                  <p className={styles.mutedText}>
                    {formatUiMessage(locale, "shareAnalyze.logsEmpty")}
                  </p>
                </div>
              )}
            </AnalyzeSection>
          </>
        )}
      </div>
    </main>
  );
}
