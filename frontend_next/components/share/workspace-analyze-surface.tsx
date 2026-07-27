"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { useAuth } from "../../lib/auth/context";
import {
  getShareAccessLogs,
  getShareAnalytics,
  getShareSettings,
  isShareEnabled,
  type AccessLogsResponse,
  type ShareAnalyticsResponse,
  type ShareSettings,
} from "../../lib/share/client";
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
        <h2 className={`app-page-title ${styles.sectionTitle}`}>
          {title}
        </h2>
        <p className="app-page-subtitle">{subtitle}</p>
      </div>
      {children}
    </section>
  );
}

export function WorkspaceAnalyzeSurface({ workspaceId }: { workspaceId: string }) {
  const auth = useAuth();
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
        setError("加载分享设置失败。");
      }

      if (analyticsResult.status === "fulfilled") {
        setAnalytics(analyticsResult.value);
      } else {
        setError((current) => current || "加载分享分析失败。");
      }

      if (logsResult.status === "fulfilled") {
        setLogs(logsResult.value);
      } else {
        setError((current) => current || "加载访问日志失败。");
      }

      setLoading(false);
    }

    void loadAnalyzeData();

    return () => {
      cancelled = true;
    };
  }, [auth.token, workspaceId]);

  return (
    <main className="app-page-shell">
      <div className={`app-page-center ${styles.pageStack}`}>
        <header className={styles.header}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceId}`}>
            返回 Workspace
          </Link>
          <div className={styles.headerRow}>
            <div>
              <h1 className="app-page-title">Analyze</h1>
              <p className="app-page-subtitle">仅展示当前 Workspace 的分享状态、访问量和最近访问记录。</p>
            </div>
            <Link className="app-button-secondary" href={`/dashboard/${workspaceId}/share`}>
              前往 Share
            </Link>
          </div>
        </header>

        {error ? <p className="app-notice-banner">{error}</p> : null}

        {loading ? (
          <section className="app-surface-card">
            <p className={styles.flushText}>正在加载分享分析...</p>
          </section>
        ) : !isShareEnabled(settings) ? (
          <AnalyzeSection
            subtitle="先在 Share 页面启用分享，再回到这里查看访问趋势和访问日志。"
            title="还没有可分析的分享数据"
          >
            <div className="app-button-row">
              <Link className="app-button-primary" href={`/dashboard/${workspaceId}/share`}>
                前往 Share
              </Link>
            </div>
          </AnalyzeSection>
        ) : (
          <>
            <AnalyzeSection
              subtitle="Analyze 页面只保留分享相关分析，不扩展成搜索页或 token 成本页。"
              title="分享状态"
            >
              <div className={styles.metricGrid}>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>访问级别</h3>
                  <p className={styles.flushText}>{settings?.access_level ?? "未设置"}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>允许下载</h3>
                  <p className={styles.flushText}>{settings?.allow_download ? "已开启" : "未开启"}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>过期时间</h3>
                  <p className={styles.flushText}>{settings?.expires_at ?? "未设置"}</p>
                </div>
              </div>
            </AnalyzeSection>

            <AnalyzeSection subtitle="总访问量和独立访客来自 share analytics。 " title="访问指标">
              <div className={styles.metricGrid}>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>总访问量</h3>
                  <p className={styles.metricValue}>{analytics?.total_views ?? 0}</p>
                </div>
                <div className="app-inline-surface">
                  <h3 className={styles.metricTitle}>独立访客</h3>
                  <p className={styles.metricValue}>
                    {analytics?.total_unique_visitors ?? 0}
                  </p>
                </div>
              </div>
            </AnalyzeSection>

            <AnalyzeSection subtitle="展示最近的分享访问动作。 " title="最近访问日志">
              {logs?.logs.length ? (
                <ul className={styles.logList}>
                  {logs.logs.slice(0, 10).map((log) => (
                    <li className="app-inline-surface" key={log.id}>
                      <strong>{log.action}</strong>
                      <div className={styles.logMeta}>
                        {log.visitor_id} · {log.accessed_at}
                      </div>
                    </li>
                  ))}
                </ul>
              ) : (
                <div className="app-inline-surface">
                  <p className={styles.mutedText}>暂时还没有访问日志。</p>
                </div>
              )}
            </AnalyzeSection>
          </>
        )}
      </div>
    </main>
  );
}
