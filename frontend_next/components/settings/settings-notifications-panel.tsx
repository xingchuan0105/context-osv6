"use client";

import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { z } from "zod";

import { describeAuthError } from "../../lib/auth/errors";
import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import {
  getUserPreferences,
  listNotifications,
  markNotificationRead,
  updateUserPreferences,
  type NotificationRow,
  type UserPreferences,
} from "../../lib/settings/client";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  TIME_24H_PATTERN,
  applyZodErrors,
  bannerStyle,
  formatDateTime,
  notificationFormDefaults,
  notificationTypeLabel,
  settingsKeys,
  type NotificationFormValues,
} from "./settings-shared";

export function NotificationsPanel() {
  const { token } = useAuth();
  const { locale } = useUiPreferences();
  const queryClient = useQueryClient();
  const notificationsForm = useForm<NotificationFormValues>({
    defaultValues: notificationFormDefaults(),
  });
  const [banner, setBanner] = useState("");
  const [actionError, setActionError] = useState("");
  const preferencesQuery = useQuery({
    queryKey: settingsKeys.preferences(token),
    enabled: Boolean(token),
    queryFn: () => getUserPreferences(token as string),
  });
  const notificationsQuery = useQuery({
    queryKey: settingsKeys.notifications(token),
    enabled: Boolean(token),
    queryFn: () => listNotifications(token as string),
  });
  const saveMutation = useMutation({
    mutationFn: async (preferences: UserPreferences) =>
      updateUserPreferences(token as string, preferences),
    onSuccess: async (updatedPreferences) => {
      queryClient.setQueryData(settingsKeys.preferences(token), updatedPreferences);
      await queryClient.invalidateQueries({ queryKey: settingsKeys.preferences(token) });
      setBanner(formatUiMessage(locale, "settings.saveSuccess"));
    },
  });
  const markReadMutation = useMutation({
    mutationFn: async (notificationId: string) => {
      await markNotificationRead(token as string, notificationId);
      return notificationId;
    },
    onSuccess: (notificationId) => {
      queryClient.setQueryData(
        settingsKeys.notifications(token),
        (current: { notifications: NotificationRow[] } | undefined) =>
          current
            ? {
                notifications: current.notifications.map((notification) =>
                  notification.id === notificationId
                    ? { ...notification, read_at: new Date().toISOString() }
                    : notification,
                ),
              }
            : current,
      );
    },
  });

  useEffect(() => {
    notificationsForm.reset(
      notificationFormDefaults(preferencesQuery.data?.notifications),
    );
  }, [notificationsForm, preferencesQuery.data]);

  const quietHoursSchema = z
    .string()
    .trim()
    .refine((value) => value.length === 0 || TIME_24H_PATTERN.test(value), {
      message: formatUiMessage(locale, "settings.notifications.invalidTime"),
    });
  const notificationSchema = z.object({
    email_enabled: z.boolean(),
    product_enabled: z.boolean(),
    security_enabled: z.boolean(),
    weekly_digest_enabled: z.boolean(),
    quiet_hours_start: quietHoursSchema,
    quiet_hours_end: quietHoursSchema,
  });

  async function handleSave(values: NotificationFormValues) {
    setBanner("");
    setActionError("");
    notificationsForm.clearErrors();

    const parsed = notificationSchema.safeParse(values);

    if (!parsed.success) {
      applyZodErrors(parsed.error, notificationsForm.setError);
      return;
    }

    if (!token) {
      setActionError(formatUiMessage(locale, "settings.profile.notAuthenticated"));
      return;
    }

    try {
      const basePreferences =
        preferencesQuery.data ?? (await getUserPreferences(token));

      await saveMutation.mutateAsync({
        ...basePreferences,
        notifications: {
          email_enabled: parsed.data.email_enabled,
          product_enabled: parsed.data.product_enabled,
          security_enabled: parsed.data.security_enabled,
          weekly_digest_enabled: parsed.data.weekly_digest_enabled,
          quiet_hours_start: parsed.data.quiet_hours_start || null,
          quiet_hours_end: parsed.data.quiet_hours_end || null,
        },
      });
    } catch (error) {
      setActionError(
        describeAuthError(
          formatUiMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  async function handleMarkRead(notificationId: string) {
    setActionError("");

    try {
      await markReadMutation.mutateAsync(notificationId);
    } catch (error) {
      setActionError(
        describeAuthError(
          formatUiMessage(locale, "settings.saveError"),
          error,
        ),
      );
    }
  }

  const loadError =
    (preferencesQuery.error &&
      describeAuthError(
        formatUiMessage(locale, "settings.loadError"),
        preferencesQuery.error,
      )) ||
    (notificationsQuery.error &&
      describeAuthError(
        formatUiMessage(locale, "settings.loadError"),
        notificationsQuery.error,
      )) ||
    "";
  const notifications = notificationsQuery.data?.notifications ?? [];

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
        <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
          <div style={{ display: "grid", gap: "0.35rem" }}>
            <h2 style={{ margin: 0 }}>
              {formatUiMessage(locale, "settings.notifications.sectionTitle")}
            </h2>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatUiMessage(locale, "settings.notifications.sectionSubtitle")}
            </p>
          </div>
          <button
            className="app-button-secondary"
            disabled={saveMutation.isPending || !token}
            type="submit"
            form="settings-notifications-form"
          >
            {saveMutation.isPending
              ? formatUiMessage(locale, "shareCenter.saving")
              : formatUiMessage(locale, "settings.notifications.saveAction")}
          </button>
        </div>
        {banner ? (
          <p className="app-notice-banner" style={bannerStyle("success")}>
            {banner}
          </p>
        ) : null}
        {actionError || loadError ? (
          <p className="app-notice-banner">{actionError || loadError}</p>
        ) : null}
        <form
          id="settings-notifications-form"
          noValidate
          style={{ display: "grid", gap: "1rem" }}
          onSubmit={notificationsForm.handleSubmit(handleSave)}
        >
          <div
            style={{
              display: "grid",
              gap: "0.75rem",
              gridTemplateColumns: "repeat(auto-fit, minmax(16rem, 1fr))",
            }}
          >
            {([
              ["email_enabled", formatUiMessage(locale, "settings.notifications.emailUpdatesLabel")],
              ["product_enabled", formatUiMessage(locale, "settings.notifications.productUpdatesLabel")],
              ["security_enabled", formatUiMessage(locale, "settings.notifications.securityAlertsLabel")],
              ["weekly_digest_enabled", formatUiMessage(locale, "settings.notifications.weeklyDigestLabel")],
            ] as const).map(([key, title]) => (
              <label
                className="app-inline-surface"
                key={key}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: "1rem",
                  cursor: "pointer",
                }}
              >
                <span>{title}</span>
                <input type="checkbox" {...notificationsForm.register(key)} />
              </label>
            ))}
          </div>
          <div
            style={{
              display: "grid",
              gap: "0.75rem",
              gridTemplateColumns: "repeat(auto-fit, minmax(16rem, 1fr))",
            }}
          >
            <div>
              <label className="app-form-label" htmlFor="settings-quiet-hours-start">
                {formatUiMessage(locale, "settings.notifications.quietHoursStartLabel")}
              </label>
              <input
                className="app-input"
                id="settings-quiet-hours-start"
                placeholder={formatUiMessage(
                  locale,
                  "settings.notifications.quietHoursPlaceholderStart",
                )}
                type="text"
                {...notificationsForm.register("quiet_hours_start")}
              />
              {notificationsForm.formState.errors.quiet_hours_start?.message ? (
                <p className="app-form-footnote" style={{ color: "hsl(var(--destructive))" }}>
                  {notificationsForm.formState.errors.quiet_hours_start.message}
                </p>
              ) : null}
            </div>
            <div>
              <label className="app-form-label" htmlFor="settings-quiet-hours-end">
                {formatUiMessage(locale, "settings.notifications.quietHoursEndLabel")}
              </label>
              <input
                className="app-input"
                id="settings-quiet-hours-end"
                placeholder={formatUiMessage(
                  locale,
                  "settings.notifications.quietHoursPlaceholderEnd",
                )}
                type="text"
                {...notificationsForm.register("quiet_hours_end")}
              />
              {notificationsForm.formState.errors.quiet_hours_end?.message ? (
                <p className="app-form-footnote" style={{ color: "hsl(var(--destructive))" }}>
                  {notificationsForm.formState.errors.quiet_hours_end.message}
                </p>
              ) : null}
            </div>
          </div>
        </form>
      </section>

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <h3 style={{ margin: 0 }}>
          {formatUiMessage(locale, "settings.notifications.historyTitle")}
        </h3>
        {notificationsQuery.isLoading ? (
          <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
            {formatUiMessage(locale, "settings.notifications.loading")}
          </p>
        ) : notifications.length === 0 ? (
          <div style={{ display: "grid", gap: "0.3rem" }}>
            <strong>
              {formatUiMessage(locale, "settings.notifications.emptyTitle")}
            </strong>
            <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
              {formatUiMessage(locale, "settings.notifications.emptyBody")}
            </p>
          </div>
        ) : (
          <div style={{ display: "grid", gap: "0.75rem" }}>
            {notifications.map((notification) => (
              <article
                className="app-inline-surface"
                key={notification.id}
                style={{
                  display: "grid",
                  gap: "0.6rem",
                  borderColor: notification.read_at ? "hsl(var(--border))" : "hsl(var(--primary))",
                }}
              >
                <div className="app-inline-row" style={{ marginBottom: 0, alignItems: "start" }}>
                  <div style={{ display: "grid", gap: "0.25rem" }}>
                    <strong>{notification.title}</strong>
                    <span style={{ color: "hsl(var(--muted-foreground))" }}>
                      {notificationTypeLabel(locale, notification.event_type)} ·{" "}
                      {formatDateTime(notification.created_at, locale)}
                    </span>
                  </div>
                  <button
                    className="app-button-ghost"
                    disabled={
                      Boolean(notification.read_at) ||
                      (markReadMutation.isPending && markReadMutation.variables === notification.id)
                    }
                    type="button"
                    onClick={() => void handleMarkRead(notification.id)}
                  >
                    {notification.read_at
                      ? formatUiMessage(locale, "settings.notifications.read")
                      : markReadMutation.isPending &&
                          markReadMutation.variables === notification.id
                        ? formatUiMessage(locale, "settings.notifications.processing")
                        : formatUiMessage(locale, "settings.notifications.markRead")}
                  </button>
                </div>
                <p style={{ margin: 0 }}>{notification.body}</p>
              </article>
            ))}
          </div>
        )}
      </section>
    </section>
  );
}

