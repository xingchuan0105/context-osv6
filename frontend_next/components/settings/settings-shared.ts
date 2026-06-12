import type { CSSProperties } from "react";
import {
  type FieldValues,
  type Path,
  type UseFormSetError,
} from "react-hook-form";
import { z } from "zod";

import { formatSettingsShareMessage } from "../../lib/settings-share-messages";
import {
  defaultNotificationPreferences,
  type NotificationPreferences,
} from "../../lib/settings/client";

export type ProfileFormValues = {
  fullName: string;
};

export type NotificationFormValues = {
  email_enabled: boolean;
  product_enabled: boolean;
  security_enabled: boolean;
  weekly_digest_enabled: boolean;
  quiet_hours_start: string;
  quiet_hours_end: string;
};

export const TIME_24H_PATTERN = /^([01]\d|2[0-3]):[0-5]\d$/;

export const settingsKeys = {
  billing: (token: string | null) => ["settings", "billing", token] as const,
  notifications: (token: string | null) => ["settings", "notifications", token] as const,
  preferences: (token: string | null) => ["settings", "preferences", token] as const,
  usageLimit: (token: string | null) => ["settings", "usage-limit", token] as const,
};

export function applyZodErrors<TFieldValues extends FieldValues>(
  error: z.ZodError<TFieldValues>,
  setError: UseFormSetError<TFieldValues>,
) {
  for (const issue of error.issues) {
    const field = issue.path[0];

    if (typeof field === "string") {
      setError(field as Path<TFieldValues>, {
        type: "manual",
        message: issue.message,
      });
    }
  }
}

export function bannerStyle(tone: "success" | "error" | "info"): CSSProperties {
  if (tone === "success") {
    return {
      border: "1px solid rgba(25, 135, 84, 0.24)",
      background: "rgba(25, 135, 84, 0.08)",
      color: "hsl(var(--success))",
    };
  }

  if (tone === "info") {
    return {
      border: "1px solid rgba(32, 124, 229, 0.18)",
      background: "rgba(32, 124, 229, 0.08)",
      color: "hsl(var(--info))",
    };
  }

  return {};
}

export function panelChoiceStyle(selected: boolean): CSSProperties {
  return {
    display: "grid",
    gap: "0.45rem",
    width: "100%",
    padding: "1rem",
    borderRadius: "1rem",
    border: `1px solid ${selected ? "hsl(var(--primary))" : "hsl(var(--border))"}`,
    background: selected ? "hsl(var(--surface-muted))" : "hsl(var(--card))",
    color: "inherit",
    textAlign: "left",
  };
}

export function progressTrackStyle(): CSSProperties {
  return {
    width: "100%",
    height: "0.5rem",
    borderRadius: "999px",
    background: "hsl(var(--muted))",
    overflow: "hidden",
  };
}

export function progressBarStyle(percent: number): CSSProperties {
  return {
    width: `${Math.max(0, Math.min(100, percent))}%`,
    height: "100%",
    borderRadius: "999px",
    background:
      percent >= 90
        ? "hsl(var(--destructive))"
        : percent >= 70
          ? "hsl(var(--warning))"
          : "hsl(var(--success))",
  };
}

export function formatDate(value: string | null, locale: "zh-CN" | "en", fallback: string) {
  if (!value?.trim()) {
    return fallback;
  }

  const timestamp = Date.parse(value);

  if (Number.isNaN(timestamp)) {
    return value;
  }

  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(new Date(timestamp));
}

export function formatDateTime(value: string, locale: "zh-CN" | "en") {
  const timestamp = Date.parse(value);

  if (Number.isNaN(timestamp)) {
    return value;
  }

  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}

export function formatCompactNumber(value: number) {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }

  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}K`;
  }

  return value.toString();
}

export function formatPrice(cents: number) {
  return `$${(cents / 100).toFixed(2)}`;
}

export function metricLabel(locale: "zh-CN" | "en", metric: string) {
  const keyMap: Record<string, Parameters<typeof formatSettingsShareMessage>[1]> = {
    embedding_tokens: "settings.metric.embedding_tokens",
    llm_input_tokens: "settings.metric.llm_input_tokens",
    llm_output_tokens: "settings.metric.llm_output_tokens",
    pages_processed: "settings.metric.pages_processed",
    storage_bytes: "settings.metric.storage_bytes",
  };

  const key = keyMap[metric.trim()];
  return key ? formatSettingsShareMessage(locale, key) : metric;
}

export function featureLabel(locale: "zh-CN" | "en", feature: string) {
  const [metric, value] = feature.split(":");

  if (!metric || !value) {
    return feature;
  }

  const normalizedValue =
    value.trim().toLowerCase() === "unlimited"
      ? formatSettingsShareMessage(locale, "commonUnlimited")
      : value.trim();

  return `${metricLabel(locale, metric)}: ${normalizedValue}`;
}

export function notificationTypeLabel(locale: "zh-CN" | "en", eventType: string) {
  const keyMap: Record<string, Parameters<typeof formatSettingsShareMessage>[1]> = {
    product_update: "settings.notifications.event.product_update",
    security_alert: "settings.notifications.event.security_alert",
    weekly_digest: "settings.notifications.event.weekly_digest",
  };

  const key = keyMap[eventType];
  return key ? formatSettingsShareMessage(locale, key) : eventType;
}

export function subscriptionStatusLabel(locale: "zh-CN" | "en", status: string) {
  const keyMap: Record<string, Parameters<typeof formatSettingsShareMessage>[1]> = {
    active: "settings.billing.status.active",
    past_due: "settings.billing.status.past_due",
    canceled: "settings.billing.status.canceled",
  };

  const key = keyMap[status];
  return key ? formatSettingsShareMessage(locale, key) : status;
}

export function notificationFormDefaults(
  preferences: NotificationPreferences = defaultNotificationPreferences(),
): NotificationFormValues {
  return {
    email_enabled: preferences.email_enabled,
    product_enabled: preferences.product_enabled,
    security_enabled: preferences.security_enabled,
    weekly_digest_enabled: preferences.weekly_digest_enabled,
    quiet_hours_start: preferences.quiet_hours_start ?? "",
    quiet_hours_end: preferences.quiet_hours_end ?? "",
  };
}

