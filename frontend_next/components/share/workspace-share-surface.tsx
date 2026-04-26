"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { useAuth } from "../../lib/auth/context";
import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import {
  buildShareUrl,
  createShareLink,
  getShareAccessLogs,
  getShareAnalytics,
  getShareSettings,
  revokeShareLink,
  type AccessLogsResponse,
  type ShareAnalyticsResponse,
  type ShareSettings,
  updateShareSettings,
} from "../../lib/share/client";
import { useUiPreferences } from "../../lib/ui-preferences";

type WorkspaceShareCenterSurfaceProps = {
  workspaceId: string;
};

type ShareStatus = "inactive" | "active" | "expired";
type ShareValidityOption = "7d" | "30d" | "90d" | "never";

const shareKeys = {
  accessLogs: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "access-logs", token] as const,
  analytics: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "analytics", token] as const,
  settings: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "settings", token] as const,
};

function hasWorkspaceId(value: string) {
  return value.trim().length > 0 && value !== "undefined";
}

function resolveValidityOption(expiresAt: string | null | undefined): ShareValidityOption {
  if (!expiresAt) {
    return "never";
  }

  const parsed = Date.parse(expiresAt);

  if (Number.isNaN(parsed)) {
    return "30d";
  }

  const deltaDays = (parsed - Date.now()) / (24 * 60 * 60 * 1000);

  if (deltaDays <= 10) {
    return "7d";
  }

  if (deltaDays <= 45) {
    return "30d";
  }

  return "90d";
}

function buildExpiresAtFromValidity(option: ShareValidityOption) {
  if (option === "never") {
    return null;
  }

  const days = option === "7d" ? 7 : option === "30d" ? 30 : 90;
  const nextDate = new Date();
  nextDate.setUTCDate(nextDate.getUTCDate() + days);
  return nextDate.toISOString();
}

function resolveShareStatus(settings: ShareSettings | null | undefined): ShareStatus | null {
  if (!settings) {
    return null;
  }

  if (!settings.share_token || settings.access_level === "private") {
    return "inactive";
  }

  if (settings.expires_at) {
    const expiresAt = Date.parse(settings.expires_at);

    if (!Number.isNaN(expiresAt) && expiresAt <= Date.now()) {
      return "expired";
    }
  }

  return "active";
}

function shareStatusLabel(locale: "zh-CN" | "en", status: ShareStatus | null) {
  if (status === "inactive") {
    return formatSettingsShareMessage(locale, "shareCenter.statusInactive");
  }

  if (status === "expired") {
    return formatSettingsShareMessage(locale, "shareCenter.statusExpired");
  }

  if (status === "active") {
    return formatSettingsShareMessage(locale, "shareCenter.statusActive");
  }

  return formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
}

function shareValidityLabel(locale: "zh-CN" | "en", option: ShareValidityOption) {
  if (option === "7d") {
    return formatSettingsShareMessage(locale, "shareCenter.validityOption7d");
  }

  if (option === "30d") {
    return formatSettingsShareMessage(locale, "shareCenter.validityOption30d");
  }

  if (option === "90d") {
    return formatSettingsShareMessage(locale, "shareCenter.validityOption90d");
  }

  return formatSettingsShareMessage(locale, "shareCenter.validityOptionNever");
}

function parseAccessedAt(value: string) {
  const numeric = Number(value);

  if (Number.isFinite(numeric) && numeric > 0) {
    return numeric > 1_000_000_000_000 ? numeric : numeric * 1000;
  }

  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? null : parsed;
}

function formatAccessedAt(locale: "zh-CN" | "en", value: string) {
  const timestamp = parseAccessedAt(value);

  if (timestamp === null) {
    return value || formatSettingsShareMessage(locale, "shareCenter.notSet");
  }

  return new Intl.DateTimeFormat(locale === "zh-CN" ? "zh-CN" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(timestamp);
}

function formatDayLabel(locale: "zh-CN" | "en", value: string) {
  const parsed = Date.parse(`${value}T00:00:00Z`);

  if (Number.isNaN(parsed)) {
    return value;
  }

  return new Intl.DateTimeFormat(locale === "zh-CN" ? "zh-CN" : "en-US", {
    day: "numeric",
    month: "short",
  }).format(parsed);
}

function buildDailyViewsSeries(
  analytics: ShareAnalyticsResponse | undefined,
  days: number,
) {
  if (!analytics) {
    return [];
  }

  const series: Array<{ day: string; views: number }> = [];
  const today = new Date();
  today.setUTCHours(0, 0, 0, 0);

  for (let offset = days - 1; offset >= 0; offset -= 1) {
    const day = new Date(today);
    day.setUTCDate(today.getUTCDate() - offset);
    const key = day.toISOString().slice(0, 10);

    series.push({
      day: key,
      views: analytics.views_by_day[key] ?? 0,
    });
  }

  return series;
}

function sumViews(series: Array<{ day: string; views: number }>) {
  return series.reduce((total, entry) => total + entry.views, 0);
}

function countActiveDays(series: Array<{ day: string; views: number }>) {
  return series.reduce((total, entry) => total + (entry.views > 0 ? 1 : 0), 0);
}

function getLatestAccessLog(logs: AccessLogsResponse | undefined) {
  if (!logs) {
    return null;
  }

  return logs.logs.reduce<AccessLogsResponse["logs"][number] | null>((latest, log) => {
    const currentTime = parseAccessedAt(log.accessed_at);
    const latestTime = latest ? parseAccessedAt(latest.accessed_at) : null;

    if (currentTime === null) {
      return latest;
    }

    if (latestTime === null || currentTime > latestTime) {
      return log;
    }

    return latest;
  }, null);
}

function SectionHeader({
  subtitle,
  title,
}: {
  subtitle: string;
  title: string;
}) {
  return (
    <div
      style={{
        borderBottom: "1px solid hsl(var(--border))",
        display: "grid",
        gap: "0.35rem",
        paddingBottom: "0.95rem",
      }}
    >
      <h2 className="app-page-title" style={{ fontSize: "1.2rem", marginBottom: 0 }}>
        {title}
      </h2>
      <p className="app-page-subtitle" style={{ margin: 0, maxWidth: "42rem" }}>
        {subtitle}
      </p>
    </div>
  );
}

function InsightMetricCard({
  title,
  value,
}: {
  title: string;
  value: string;
}) {
  return (
    <section
      className="app-inline-surface"
      style={{
        display: "grid",
        gap: "0.55rem",
        minHeight: "clamp(5.8rem, 18vw, 7.25rem)",
        padding: "clamp(0.9rem, 2.5vw, 1rem) clamp(0.9rem, 2.5vw, 1rem) clamp(0.95rem, 2.5vw, 1.05rem)",
      }}
    >
      <h3
        style={{
          color: "hsl(var(--muted-foreground))",
          fontSize: "0.88rem",
          fontWeight: 600,
          letterSpacing: "-0.01em",
          margin: 0,
        }}
      >
        {title}
      </h3>
      <p
        style={{
          fontSize: "clamp(1.4rem, 4.8vw, 1.85rem)",
          fontWeight: 700,
          letterSpacing: "-0.03em",
          lineHeight: 1.05,
          margin: 0,
        }}
      >
        {value}
      </p>
    </section>
  );
}

function shareStatusBadgeStyle(status: ShareStatus | null) {
  if (status === "active") {
    return {
      background: "hsl(var(--primary) / 0.12)",
      border: "1px solid hsl(var(--primary) / 0.18)",
      color: "hsl(var(--primary))",
    };
  }

  if (status === "expired") {
    return {
      background: "hsl(var(--destructive) / 0.1)",
      border: "1px solid hsl(var(--destructive) / 0.18)",
      color: "hsl(var(--destructive))",
    };
  }

  return {
    background: "hsl(var(--muted))",
    border: "1px solid hsl(var(--border))",
    color: "hsl(var(--muted-foreground))",
  };
}

export function WorkspaceShareCenterSurface({
  workspaceId,
}: WorkspaceShareCenterSurfaceProps) {
  const auth = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();
  const workspaceReady = hasWorkspaceId(workspaceId);
  const invalidWorkspaceMessage =
    locale === "zh-CN" ? "当前工作区标识无效。" : "Invalid workspace identifier.";
  const [actionError, setActionError] = useState("");
  const [actionMessage, setActionMessage] = useState("");
  const [expiresAtDraft, setExpiresAtDraft] = useState<ShareValidityOption>("30d");
  const settingsQuery = useQuery({
    queryKey: shareKeys.settings(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => getShareSettings(auth.token as string, workspaceId),
  });
  const analyticsQuery = useQuery({
    queryKey: shareKeys.analytics(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => getShareAnalytics(auth.token as string, workspaceId),
  });
  const accessLogsQuery = useQuery({
    queryKey: shareKeys.accessLogs(workspaceId, auth.token),
    enabled: Boolean(auth.token && workspaceReady),
    queryFn: () => getShareAccessLogs(auth.token as string, workspaceId),
  });
  const toggleShareMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      const currentSettings = settingsQuery.data;
      const currentStatus = resolveShareStatus(currentSettings ?? null);

      if (currentStatus === "active" && currentSettings?.share_token) {
        await revokeShareLink(auth.token, workspaceId, currentSettings.share_token);
        return updateShareSettings(auth.token, workspaceId, {
          access_level: "private",
          allow_download: false,
        });
      }

      if (currentSettings?.share_token) {
        await revokeShareLink(auth.token, workspaceId, currentSettings.share_token);
      }

      if (!currentSettings?.share_token || currentStatus !== "active") {
        await createShareLink(auth.token, workspaceId, {
          role: "viewer",
          expires_at: buildExpiresAtFromValidity(expiresAtDraft),
        });
        return updateShareSettings(auth.token, workspaceId, {
          access_level: "link",
          allow_download: false,
        });
      }

      return currentSettings;
    },
    onSuccess: async (settings) => {
      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
    },
  });
  const refreshShareMutation = useMutation({
    mutationFn: async () => {
      if (!auth.token) {
        throw new Error(formatSettingsShareMessage(locale, "shareCenter.loginRequired"));
      }

      if (!workspaceReady) {
        throw new Error(invalidWorkspaceMessage);
      }

      const currentSettings = settingsQuery.data;
      const nextExpiresAt = buildExpiresAtFromValidity(expiresAtDraft);

      if (currentSettings?.share_token) {
        await revokeShareLink(auth.token, workspaceId, currentSettings.share_token);
      }

      await createShareLink(auth.token, workspaceId, {
        role: "viewer",
        expires_at: nextExpiresAt,
      });

      return updateShareSettings(auth.token, workspaceId, {
        access_level: "link",
        allow_download: false,
      });
    },
    onSuccess: async (settings) => {
      queryClient.setQueryData(shareKeys.settings(workspaceId, auth.token), settings);
      await queryClient.invalidateQueries({
        queryKey: shareKeys.settings(workspaceId, auth.token),
      });
    },
  });
  useEffect(() => {
    if (!settingsQuery.data) {
      return;
    }

    setExpiresAtDraft(resolveValidityOption(settingsQuery.data.expires_at));
  }, [settingsQuery.data]);

  const shareUrl = buildShareUrl(settingsQuery.data?.share_token ?? "");
  const hasShareLink = Boolean(settingsQuery.data?.share_token);
  const shareStatus = resolveShareStatus(settingsQuery.data ?? null);
  const shareStatusText = shareStatusLabel(locale, shareStatus);
  const sevenDaySeries = buildDailyViewsSeries(analyticsQuery.data, 7);
  const thirtyDaySeries = buildDailyViewsSeries(analyticsQuery.data, 30);
  const [trendWindowDays, setTrendWindowDays] = useState<7 | 30>(7);
  const trendSeries = trendWindowDays === 7 ? sevenDaySeries : thirtyDaySeries;
  const totalViewsValue =
    analyticsQuery.data?.total_views.toLocaleString() ??
    formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const recentViewsValue = analyticsQuery.data
    ? sumViews(sevenDaySeries).toLocaleString()
    : formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const activeDaysValue = analyticsQuery.data
    ? String(countActiveDays(thirtyDaySeries))
    : formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const latestAccessLog = getLatestAccessLog(accessLogsQuery.data);
  const latestAccessValue = accessLogsQuery.data
    ? latestAccessLog
      ? formatAccessedAt(locale, latestAccessLog.accessed_at)
      : formatSettingsShareMessage(locale, "shareCenter.notSet")
    : formatSettingsShareMessage(locale, "shareCenter.metricUnavailable");
  const canUseShareLink = shareStatus === "active" && Boolean(shareUrl);
  const shareSwitchChecked = shareStatus === "active";
  const validityOptions: ShareValidityOption[] = ["7d", "30d", "90d", "never"];

  async function handleToggleShare() {
    setActionError("");
    setActionMessage("");

    try {
      await toggleShareMutation.mutateAsync();
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : formatSettingsShareMessage(locale, "shareCenter.saveError"),
      );
    }
  }


  async function handleCopyShareLink() {
    setActionError("");
    setActionMessage("");

    if (!canUseShareLink) {
      setActionError(formatSettingsShareMessage(locale, "shareCenter.shareLinkUnavailable"));
      return;
    }

    try {
      await navigator.clipboard.writeText(shareUrl);
      setActionMessage(formatSettingsShareMessage(locale, "shareCenter.copyLinkSuccess"));
    } catch {
      setActionError(formatSettingsShareMessage(locale, "shareCenter.copyLinkError"));
    }
  }

  function handleOpenSharePage() {
    setActionError("");
    setActionMessage("");

    if (!canUseShareLink) {
      setActionError(formatSettingsShareMessage(locale, "shareCenter.shareLinkUnavailable"));
      return;
    }

    window.open(shareUrl, "_blank", "noopener,noreferrer");
  }

  async function handleRefreshShare() {
    setActionError("");
    setActionMessage("");

    try {
      await refreshShareMutation.mutateAsync();
      setActionMessage(formatSettingsShareMessage(locale, "shareCenter.updateShareSuccess"));
    } catch (error) {
      setActionError(
        error instanceof Error
          ? error.message
          : formatSettingsShareMessage(locale, "shareCenter.saveError"),
      );
    }
  }

  return (
    <main className="app-page-shell">
      <div
        className="app-page-center"
        style={{ display: "grid", gap: "0.85rem", maxWidth: "54rem", width: "100%" }}
      >
        <header style={{ display: "grid", gap: "0.65rem" }}>
          <Link className="app-link app-link-muted" href={`/dashboard/${workspaceId}`}>
            {formatSettingsShareMessage(locale, "shareCenter.backToWorkspace")}
          </Link>
          <div
            style={{
              alignItems: "start",
              display: "grid",
              gap: "1rem",
              gridTemplateColumns: "minmax(0, 1fr)",
            }}
          >
            <div>
              <h1 className="app-page-title" style={{ fontSize: "clamp(2.15rem, 5vw, 2.8rem)" }}>
                {formatSettingsShareMessage(locale, "shareCenter.pageTitle")}
              </h1>
              <p
                className="app-page-subtitle"
                style={{ fontSize: "1rem", lineHeight: 1.55, marginTop: "0.25rem" }}
              >
                {formatSettingsShareMessage(locale, "shareCenter.pageSubtitle")}
              </p>
            </div>
            <section
              className="app-surface-card"
              style={{
                display: "grid",
                gap: "0.85rem",
                padding: "0.9rem 0.95rem 0.95rem",
              }}
            >
              <div style={{ display: "grid", gap: "0.25rem" }}>
                <div
                  style={{
                    alignItems: "center",
                    display: "flex",
                    flexWrap: "wrap",
                    gap: "0.6rem",
                    justifyContent: "space-between",
                  }}
                >
                  <strong>{formatSettingsShareMessage(locale, "shareCenter.controlBarTitle")}</strong>
                  <span
                    style={{
                      ...shareStatusBadgeStyle(shareStatus),
                      borderRadius: "999px",
                      fontSize: "0.76rem",
                      fontWeight: 600,
                      letterSpacing: "-0.01em",
                      padding: "0.28rem 0.62rem",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {shareStatusText}
                  </span>
                </div>
                <p
                  style={{
                    color: "hsl(var(--muted-foreground))",
                    margin: 0,
                    fontSize: "0.96rem",
                    lineHeight: 1.5,
                  }}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.controlBarSubtitle")}
                </p>
              </div>

              <div
                className="app-inline-surface"
                style={{
                  display: "grid",
                  gap: "0.8rem",
                  padding: "0.78rem 0.88rem 0.82rem",
                }}
              >
                <div
                  style={{
                    alignItems: "center",
                    display: "flex",
                    flexWrap: "wrap",
                    gap: "0.8rem",
                    justifyContent: "space-between",
                  }}
                >
                  <div style={{ display: "grid", gap: "0.2rem" }}>
                    <span style={{ color: "hsl(var(--muted-foreground))", fontSize: "0.82rem" }}>
                      {formatSettingsShareMessage(locale, "shareCenter.shareSwitchLabel")}
                    </span>
                    <strong style={{ fontSize: "0.92rem", letterSpacing: "-0.01em" }}>
                      {shareSwitchChecked
                        ? formatSettingsShareMessage(locale, "shareCenter.statusActive")
                        : formatSettingsShareMessage(locale, "shareCenter.statusInactive")}
                    </strong>
                  </div>
                  <button
                    aria-checked={shareSwitchChecked}
                    className="app-button-ghost"
                    disabled={toggleShareMutation.isPending || settingsQuery.isLoading}
                    role="switch"
                    style={{
                      alignItems: "center",
                      background: shareSwitchChecked
                        ? "hsl(var(--foreground))"
                        : "hsl(var(--muted))",
                      border: "1px solid hsl(var(--border))",
                      borderRadius: "999px",
                      display: "inline-flex",
                      height: "2rem",
                      justifyContent: shareSwitchChecked ? "flex-end" : "flex-start",
                      minWidth: "3.55rem",
                      padding: "0.16rem",
                    }}
                    type="button"
                    onClick={() => void handleToggleShare()}
                  >
                    <span
                      aria-hidden="true"
                      style={{
                        background: shareSwitchChecked
                          ? "hsl(var(--background))"
                          : "hsl(var(--foreground))",
                        borderRadius: "999px",
                        display: "block",
                        height: "1.52rem",
                        width: "1.52rem",
                      }}
                    />
                  </button>
                </div>
                <div style={{ display: "grid", gap: "0.35rem" }}>
                  <label className="app-form-label" htmlFor="share-validity">
                    {formatSettingsShareMessage(locale, "shareCenter.validityLabel")}
                  </label>
                  <select
                    className="app-input"
                    disabled={toggleShareMutation.isPending || refreshShareMutation.isPending}
                    id="share-validity"
                    value={expiresAtDraft}
                    onChange={(event) =>
                      setExpiresAtDraft(event.target.value as ShareValidityOption)
                    }
                  >
                    {validityOptions.map((option) => (
                      <option key={option} value={option}>
                        {shareValidityLabel(locale, option)}
                      </option>
                    ))}
                  </select>
                  <p className="app-form-footnote" style={{ fontSize: "0.82rem", margin: 0 }}>
                    {formatSettingsShareMessage(locale, "shareCenter.validityHint")}
                  </p>
                </div>
                <div
                  style={{
                    color: "hsl(var(--muted-foreground))",
                    display: "grid",
                    gap: "0.3rem",
                  }}
                >
                  <span style={{ fontSize: "0.82rem" }}>
                    {formatSettingsShareMessage(locale, "shareCenter.shareUrlLabel")}
                  </span>
                  <div
                    style={{
                      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                      fontSize: "0.84rem",
                      lineHeight: 1.5,
                      overflowWrap: "anywhere",
                    }}
                  >
                    {shareUrl ||
                      formatSettingsShareMessage(locale, "shareCenter.controlBarNoLink")}
                  </div>
                </div>
              </div>

              <div
                className="app-button-row"
                style={{
                  display: "grid",
                  gap: "0.6rem",
                  gridTemplateColumns: "minmax(0, 1fr)",
                }}
              >
                <button
                  className="app-button-ghost"
                  disabled={!canUseShareLink}
                  style={{
                    fontSize: "0.9rem",
                    justifyContent: "center",
                    minHeight: "2.4rem",
                    padding: "0.62rem 0.8rem",
                    width: "100%",
                  }}
                  type="button"
                  onClick={() => void handleCopyShareLink()}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.copyLinkAction")}
                </button>
                <button
                  className="app-button-secondary"
                  disabled={!canUseShareLink}
                  style={{
                    fontSize: "0.9rem",
                    justifyContent: "center",
                    minHeight: "2.4rem",
                    padding: "0.62rem 0.8rem",
                    width: "100%",
                  }}
                  type="button"
                  onClick={() => handleOpenSharePage()}
                >
                  {formatSettingsShareMessage(locale, "shareCenter.openShareAction")}
                </button>
                <button
                  className="app-button-primary"
                  disabled={
                    refreshShareMutation.isPending ||
                    settingsQuery.isLoading ||
                    !settingsQuery.data?.share_token
                  }
                  style={{
                    fontSize: "0.9rem",
                    justifyContent: "center",
                    minHeight: "2.4rem",
                    padding: "0.62rem 0.8rem",
                    width: "100%",
                  }}
                  type="button"
                  onClick={() => void handleRefreshShare()}
                >
                  {refreshShareMutation.isPending
                    ? formatSettingsShareMessage(locale, "shareCenter.saving")
                    : formatSettingsShareMessage(locale, "shareCenter.updateShareAction")}
                </button>
              </div>
            </section>
          </div>
        </header>

        {actionError ? (
          <p className="app-notice-banner">{actionError}</p>
        ) : null}

        {actionMessage ? (
          <p className="app-inline-surface" style={{ margin: 0 }}>
            {actionMessage}
          </p>
        ) : null}

        {settingsQuery.isLoading && !settingsQuery.data ? (
          <section className="app-surface-card">
            <p style={{ margin: 0 }}>
              {formatSettingsShareMessage(locale, "shareCenter.loading")}
            </p>
          </section>
        ) : null}

        {settingsQuery.error && !settingsQuery.data ? (
          <section className="app-surface-card">
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {settingsQuery.error instanceof Error
                ? settingsQuery.error.message
                : formatSettingsShareMessage(locale, "shareCenter.settingsLoadError")}
            </p>
          </section>
        ) : null}

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

        {accessLogsQuery.isLoading && !accessLogsQuery.data ? (
          <section className="app-surface-card" id="activity" style={{ scrollMarginTop: "6rem" }}>
            <p style={{ margin: 0 }}>
              {formatSettingsShareMessage(locale, "shareCenter.accessLogsLoading")}
            </p>
          </section>
        ) : accessLogsQuery.error && !accessLogsQuery.data ? (
          <section className="app-surface-card" id="activity" style={{ scrollMarginTop: "6rem" }}>
            <p className="app-notice-banner" style={{ margin: 0 }}>
              {accessLogsQuery.error instanceof Error
                ? accessLogsQuery.error.message
                : formatSettingsShareMessage(locale, "shareCenter.accessLogsLoadError")}
            </p>
          </section>
        ) : (
          <section
            className="app-surface-card"
            id="activity"
            style={{
              background:
                "linear-gradient(180deg, hsl(var(--background)) 0%, hsl(var(--muted) / 0.24) 100%)",
              display: "grid",
              gap: "0.95rem",
              padding: "0.95rem 1rem 1rem",
              scrollMarginTop: "6rem",
            }}
          >
            <SectionHeader
              subtitle={formatSettingsShareMessage(locale, "shareCenter.activitySectionSubtitle")}
              title={formatSettingsShareMessage(locale, "shareCenter.activitySectionTitle")}
            />

            {accessLogsQuery.data && accessLogsQuery.data.logs.length > 0 ? (
              <div style={{ display: "grid", gap: "0.75rem" }}>
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
                      className="app-inline-surface"
                      key={log.id}
                      style={{
                        borderLeft: "3px solid hsl(var(--primary) / 0.24)",
                        display: "grid",
                        gap: "0.6rem",
                        gridTemplateColumns: "minmax(0, 1fr)",
                        padding: "0.72rem 0.82rem 0.78rem",
                      }}
                    >
                      <div style={{ display: "grid", gap: "0.2rem" }}>
                        <span style={{ color: "hsl(var(--muted-foreground))" }}>
                          {formatSettingsShareMessage(locale, "shareCenter.activityActionLabel")}
                        </span>
                        <strong>{log.action}</strong>
                      </div>
                      <div style={{ display: "grid", gap: "0.2rem" }}>
                        <span style={{ color: "hsl(var(--muted-foreground))" }}>
                          {formatSettingsShareMessage(locale, "shareCenter.activityTimeLabel")}
                        </span>
                        <span>{formatAccessedAt(locale, log.accessed_at)}</span>
                      </div>
                    </div>
                  ))}
              </div>
            ) : (
              <div className="app-inline-surface" style={{ display: "grid", gap: "0.25rem" }}>
                <strong>{formatSettingsShareMessage(locale, "shareCenter.activityEmptyTitle")}</strong>
                <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
                  {formatSettingsShareMessage(locale, "shareCenter.activityEmptyBody")}
                </p>
              </div>
            )}
          </section>
        )}

      </div>
    </main>
  );
}
