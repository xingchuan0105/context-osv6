// @vitest-environment jsdom

import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { NotificationCenter } from './notification-center';
import { notificationApi } from '@/lib/api/client';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) =>
      typeof options?.count === 'number' ? `${key}:${options.count}` : key,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/lib/api/client', () => ({
  notificationApi: {
    listNotifications: vi.fn(),
    markNotificationRead: vi.fn(),
  },
}));

describe('NotificationCenter', () => {
  const listNotificationsMock = vi.mocked(notificationApi.listNotifications);
  const markNotificationReadMock = vi.mocked(notificationApi.markNotificationRead);

  beforeEach(() => {
    vi.clearAllMocks();
    listNotificationsMock.mockResolvedValue({
      success: true,
      data: [
        {
          id: 'notif-1',
          org_id: 'org-1',
          user_id: 'user-1',
          event_type: 'notebook_invite',
          title: 'Notebook invite',
          body: 'You were invited to Notebook A.',
          data: {},
          created_at: '2026-03-17T08:00:00.000Z',
          updated_at: '2026-03-17T08:00:00.000Z',
        },
        {
          id: 'notif-2',
          org_id: 'org-1',
          user_id: 'user-1',
          event_type: 'document_ready',
          title: 'Document ready',
          body: 'Your upload completed.',
          data: {},
          read_at: '2026-03-17T09:00:00.000Z',
          created_at: '2026-03-17T09:00:00.000Z',
          updated_at: '2026-03-17T09:00:00.000Z',
        },
      ],
    });
    markNotificationReadMock.mockResolvedValue({
      success: true,
      data: { status: 'ok' },
    });
  });

  it('shows unread badge, opens modal, and marks notifications as read', async () => {
    const user = userEvent.setup();

    render(<NotificationCenter />);

    await waitFor(() => {
      expect(listNotificationsMock).toHaveBeenCalledWith(20, 0);
    });

    expect(screen.getByLabelText('notifications.unreadCount:1')).toBeTruthy();

    await user.click(screen.getByTitle('notifications.title'));

    await screen.findByText('Notebook invite');
    await user.click(screen.getByRole('button', { name: 'notifications.markRead' }));

    await waitFor(() => {
      expect(markNotificationReadMock).toHaveBeenCalledWith('notif-1');
    });

    await waitFor(() => {
      expect(screen.queryByLabelText('notifications.unreadCount:1')).toBeNull();
    });
  });
});
