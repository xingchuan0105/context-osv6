"use client";

import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useAuth } from "../../lib/auth/context";
import { formatUiMessage } from "../../lib/i18n/messages";
import type { UiLocale } from "../../lib/i18n/config";
import {
  listNotifications,
  markNotificationRead,
  type NotificationRow,
} from "../../lib/settings/client";
import {
  formatDateTime,
  notificationTypeLabel,
  settingsKeys,
} from "../settings/settings-shared";
import styles from "./notification-bell.module.css";

/**
 * Account-level notification bell (W4 #11).
 * Token width 384–400px; items ~60–64px row.
 */
export function NotificationBell({ locale }: { locale: UiLocale }) {
  const { token } = useAuth();
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const notificationsQuery = useQuery({
    queryKey: settingsKeys.notifications(token),
    enabled: Boolean(token),
    refetchInterval: open ? 30_000 : 60_000,
    queryFn: () => listNotifications(token as string),
  });

  const markReadMutation = useMutation({
    mutationFn: async (id: string) => {
      await markNotificationRead(token as string, id);
      return id;
    },
    onSuccess: (id) => {
      queryClient.setQueryData(
        settingsKeys.notifications(token),
        (current: { notifications: NotificationRow[] } | undefined) =>
          current
            ? {
                notifications: current.notifications.map((n) =>
                  n.id === id ? { ...n, read_at: new Date().toISOString() } : n,
                ),
              }
            : current,
      );
    },
  });

  useEffect(() => {
    if (!open) {
      return;
    }
    function onPointerDown(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  if (!token) {
    return null;
  }

  const items = notificationsQuery.data?.notifications ?? [];
  const unread = items.filter((n) => !n.read_at).length;

  return (
    <div className={styles.root} ref={rootRef}>
      <button
        aria-expanded={open}
        aria-haspopup="dialog"
        aria-label={locale === "zh-CN" ? "通知" : "Notifications"}
        className={`${styles.trigger} top-bar-capsule`}
        data-testid="notification-bell"
        type="button"
        onClick={() => setOpen((v) => !v)}
      >
        <span className={styles.bellIcon} aria-hidden="true">
          🔔
        </span>
        {unread > 0 ? (
          <span className={styles.badge} data-testid="notification-bell-unread">
            {unread > 99 ? "99+" : unread}
          </span>
        ) : null}
      </button>
      {open ? (
        <div
          className={styles.panel}
          data-testid="notification-bell-panel"
          role="dialog"
          aria-label={locale === "zh-CN" ? "通知列表" : "Notification list"}
        >
          <header className={styles.panelHeader}>
            <strong>{locale === "zh-CN" ? "通知" : "Notifications"}</strong>
            {unread > 0 ? (
              <span className={styles.unreadHint}>
                {locale === "zh-CN" ? `${unread} 条未读` : `${unread} unread`}
              </span>
            ) : null}
          </header>
          <div className={styles.list}>
            {notificationsQuery.isLoading ? (
              <p className={styles.empty}>
                {formatUiMessage(locale, "settings.notifications.loading")}
              </p>
            ) : items.length === 0 ? (
              <div className={styles.empty}>
                <strong>
                  {formatUiMessage(locale, "settings.notifications.emptyTitle")}
                </strong>
                <p>{formatUiMessage(locale, "settings.notifications.emptyBody")}</p>
              </div>
            ) : (
              items.slice(0, 30).map((n) => (
                <article
                  className={`${styles.item}${n.read_at ? "" : ` ${styles.itemUnread}`}`}
                  key={n.id}
                  data-testid="notification-item"
                >
                  <div className={styles.itemTop}>
                    <div className={styles.itemMeta}>
                      <strong className={styles.itemTitle}>{n.title}</strong>
                      <span className={styles.itemSub}>
                        {notificationTypeLabel(locale, n.event_type)} ·{" "}
                        {formatDateTime(n.created_at, locale)}
                      </span>
                    </div>
                    {!n.read_at ? (
                      <button
                        className={styles.markRead}
                        disabled={
                          markReadMutation.isPending && markReadMutation.variables === n.id
                        }
                        type="button"
                        onClick={() => void markReadMutation.mutateAsync(n.id)}
                      >
                        {formatUiMessage(locale, "settings.notifications.markRead")}
                      </button>
                    ) : (
                      <span className={styles.readTag}>
                        {formatUiMessage(locale, "settings.notifications.read")}
                      </span>
                    )}
                  </div>
                  <p className={styles.itemBody}>{n.body}</p>
                </article>
              ))
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
