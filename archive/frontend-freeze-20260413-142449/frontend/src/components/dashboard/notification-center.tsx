'use client';

import { useEffect, useMemo, useState } from 'react';
import { Bell, CheckCheck, Loader2, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { notificationApi } from '@/lib/api/client';
import type { Notification } from '@/types';
import { toast } from '@/components/ui/toaster';

function formatNotificationTime(value: string): string {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
}

function mergeNotifications(
  incoming: Notification[],
  current: Notification[],
): Notification[] {
  if (current.length === 0) {
    return incoming;
  }
  const readAtByID = new Map(
    current
      .filter((item) => item.read_at)
      .map((item) => [item.id, item.read_at] as const),
  );
  return incoming.map((item) => {
    const localReadAt = readAtByID.get(item.id);
    if (localReadAt && !item.read_at) {
      return { ...item, read_at: localReadAt };
    }
    return item;
  });
}

export function NotificationCenter() {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [markingID, setMarkingID] = useState<string | null>(null);
  const loadFailedMessage = t('notifications.loadFailed');

  useEffect(() => {
    let cancelled = false;

    const loadNotifications = async () => {
      setLoading(true);
      try {
        const response = await notificationApi.listNotifications(20, 0);
        if (cancelled) {
          return;
        }
        if (!response.success) {
          toast.error(response.error || loadFailedMessage);
          return;
        }
        setNotifications((current) =>
          mergeNotifications(response.data || [], current),
        );
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void loadNotifications();
    return () => {
      cancelled = true;
    };
  }, [loadFailedMessage]);

  const unreadCount = useMemo(
    () => notifications.filter((item) => !item.read_at).length,
    [notifications],
  );

  const handleMarkRead = async (notificationID: string) => {
    setMarkingID(notificationID);
    try {
      const response = await notificationApi.markNotificationRead(notificationID);
      if (!response.success) {
        toast.error(response.error || t('notifications.markReadFailed'));
        return;
      }
      setNotifications((prev) =>
        prev.map((item) =>
          item.id === notificationID
            ? { ...item, read_at: item.read_at || new Date().toISOString() }
            : item,
        ),
      );
    } finally {
      setMarkingID(null);
    }
  };

  return (
    <>
      <button
        onClick={() => setOpen(true)}
        className="relative p-2 rounded-lg hover:bg-accent transition-colors text-muted-foreground hover:text-foreground"
        title={t('notifications.title')}
        type="button"
      >
        <Bell className="w-5 h-5" />
        {unreadCount > 0 && (
          <span
            className="absolute -right-1 -top-1 min-w-[18px] rounded-full bg-primary px-1.5 py-0.5 text-[10px] font-semibold leading-none text-primary-foreground"
            aria-label={t('notifications.unreadCount', { count: unreadCount })}
          >
            {unreadCount > 9 ? '9+' : unreadCount}
          </span>
        )}
      </button>

      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
          <div className="w-full max-w-2xl rounded-3xl border border-border bg-card/94 shadow-[var(--shadow-lg)] backdrop-blur-xl">
            <div className="flex items-center justify-between gap-3 border-b border-border p-4">
              <div>
                <h3 className="text-base font-semibold">{t('notifications.title')}</h3>
                <p className="text-sm text-muted-foreground">
                  {t('notifications.subtitle')}
                </p>
              </div>
              <button
                className="p-2 rounded-lg hover:bg-accent"
                onClick={() => setOpen(false)}
                aria-label={t('common.close')}
                type="button"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="max-h-[70vh] overflow-y-auto p-4">
              {loading ? (
                <div className="flex min-h-[220px] items-center justify-center text-muted-foreground">
                  <Loader2 className="mr-2 h-5 w-5 animate-spin" />
                  <span>{t('notifications.loading')}</span>
                </div>
              ) : notifications.length === 0 ? (
                <div className="flex min-h-[220px] items-center justify-center rounded-2xl border border-dashed border-border bg-background/40 text-sm text-muted-foreground">
                  {t('notifications.empty')}
                </div>
              ) : (
                <div className="space-y-3">
                  {notifications.map((item) => {
                    const unread = !item.read_at;
                    return (
                      <div
                        key={item.id}
                        className={`rounded-2xl border p-4 shadow-[var(--shadow-sm)] transition-colors ${
                          unread
                            ? 'border-primary/30 bg-primary/5'
                            : 'border-border bg-background/45'
                        }`}
                      >
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0 space-y-1">
                            <div className="flex flex-wrap items-center gap-2">
                              <h4 className="text-sm font-semibold text-foreground">
                                {item.title}
                              </h4>
                              {unread && (
                                <span className="rounded-full bg-primary/12 px-2 py-0.5 text-[11px] font-medium text-primary">
                                  {t('notifications.unread')}
                                </span>
                              )}
                            </div>
                            <p className="text-sm text-muted-foreground">{item.body}</p>
                            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                              <span>{formatNotificationTime(item.created_at)}</span>
                              <span className="uppercase tracking-[0.18em]">
                                {item.event_type.replaceAll('_', ' ')}
                              </span>
                            </div>
                          </div>
                          {unread ? (
                            <button
                              type="button"
                              onClick={() => void handleMarkRead(item.id)}
                              disabled={markingID === item.id}
                              className="inline-flex shrink-0 items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm hover:bg-accent disabled:opacity-60"
                            >
                              {markingID === item.id ? (
                                <Loader2 className="h-4 w-4 animate-spin" />
                              ) : (
                                <CheckCheck className="h-4 w-4" />
                              )}
                              {t('notifications.markRead')}
                            </button>
                          ) : (
                            <span className="text-xs text-muted-foreground">
                              {t('notifications.read')}
                            </span>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
