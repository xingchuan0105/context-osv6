import { formatSettingsShareMessage } from "../../../lib/settings-share-messages";
import type { AccessLogsResponse, MemberRow, ShareAnalyticsResponse, ShareSettings } from "../../../lib/share/client";

type WorkspaceShareCenterSurfaceProps = {
  workspaceId: string;
};

export type ShareStatus = "inactive" | "active" | "expired";
export type ShareValidityOption = "7d" | "30d" | "90d" | "never";

export const shareKeys = {
  accessLogs: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "access-logs", token] as const,
  analytics: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "analytics", token] as const,
  members: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "members", token] as const,
  settings: (workspaceId: string, token: string | null) =>
    ["share-center", workspaceId, "settings", token] as const,
};

export function hasWorkspaceId(value: string) {
  return value.trim().length > 0 && value !== "undefined";
}

export function resolveValidityOption(expiresAt: string | null | undefined): ShareValidityOption {
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

export function buildExpiresAtFromValidity(option: ShareValidityOption) {
  if (option === "never") {
    return null;
  }

  const days = option === "7d" ? 7 : option === "30d" ? 30 : 90;
  const nextDate = new Date();
  nextDate.setUTCDate(nextDate.getUTCDate() + days);
  return nextDate.toISOString();
}

export function resolveShareStatus(settings: ShareSettings | null | undefined): ShareStatus | null {
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

export function shareStatusLabel(locale: "zh-CN" | "en", status: ShareStatus | null) {
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

export function shareValidityLabel(locale: "zh-CN" | "en", option: ShareValidityOption) {
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

export function parseAccessedAt(value: string) {
  const numeric = Number(value);

  if (Number.isFinite(numeric) && numeric > 0) {
    return numeric > 1_000_000_000_000 ? numeric : numeric * 1000;
  }

  const parsed = Date.parse(value);
  return Number.isNaN(parsed) ? null : parsed;
}

export function formatAccessedAt(locale: "zh-CN" | "en", value: string) {
  const timestamp = parseAccessedAt(value);

  if (timestamp === null) {
    return value || formatSettingsShareMessage(locale, "shareCenter.notSet");
  }

  return new Intl.DateTimeFormat(locale === "zh-CN" ? "zh-CN" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(timestamp);
}

export function formatDayLabel(locale: "zh-CN" | "en", value: string) {
  const parsed = Date.parse(`${value}T00:00:00Z`);

  if (Number.isNaN(parsed)) {
    return value;
  }

  return new Intl.DateTimeFormat(locale === "zh-CN" ? "zh-CN" : "en-US", {
    day: "numeric",
    month: "short",
  }).format(parsed);
}

export function buildDailyViewsSeries(
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

export function sumViews(series: Array<{ day: string; views: number }>) {
  return series.reduce((total, entry) => total + entry.views, 0);
}

export function countActiveDays(series: Array<{ day: string; views: number }>) {
  return series.reduce((total, entry) => total + (entry.views > 0 ? 1 : 0), 0);
}

export function getLatestAccessLog(logs: AccessLogsResponse | undefined) {
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

export function memberRoleLabel(locale: "zh-CN" | "en", role: string) {
  if (role === "editor") {
    return formatSettingsShareMessage(locale, "shareCenter.memberRole.editor");
  }

  if (role === "owner") {
    return formatSettingsShareMessage(locale, "shareCenter.memberRole.owner");
  }

  return formatSettingsShareMessage(locale, "shareCenter.memberRole.viewer");
}

export function memberStatusLabel(locale: "zh-CN" | "en", status: string) {
  if (status === "accepted") {
    return formatSettingsShareMessage(locale, "shareCenter.memberStatus.accepted");
  }

  if (status === "revoked") {
    return formatSettingsShareMessage(locale, "shareCenter.memberStatus.revoked");
  }

  return formatSettingsShareMessage(locale, "shareCenter.memberStatus.pending");
}

export function memberDisplayName(member: MemberRow) {
  return member.email.trim() || member.user_id || member.member_id;
}

export function isValidInviteEmail(value: string) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim());
}