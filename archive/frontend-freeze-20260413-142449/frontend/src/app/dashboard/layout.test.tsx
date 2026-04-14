// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DashboardLayout from './layout';
import { useAppStore } from '@/stores/useAppStore';
import { kbApi } from '@/lib/api/client';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('next/navigation', () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn() }),
  usePathname: () => '/dashboard/kb-1',
}));

vi.mock('@/stores/useAppStore', () => ({
  useAppStore: vi.fn(),
}));

vi.mock('@/components/omnibar/search-dialog', () => ({
  SearchDialog: () => <div data-testid="search-dialog" />,
}));

vi.mock('@/components/settings/settings-drawer', () => ({
  SettingsDrawer: () => <div data-testid="settings-drawer" />,
}));

vi.mock('@/components/dashboard/api-access-modal', () => ({
  APIAccessModal: () => <div data-testid="api-access-modal" />,
}));

vi.mock('@/components/dashboard/notification-center', () => ({
  NotificationCenter: () => <div data-testid="notification-center" />,
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/lib/api/client', () => ({
  authApi: {
    me: vi.fn(),
  },
  clearAuthToken: vi.fn(),
  getAuthToken: vi.fn(() => 'test-token'),
  kbApi: {
    createShare: vi.fn(),
    getShareSettings: vi.fn(),
    updateAccessLevel: vi.fn(),
    inviteMember: vi.fn(),
    removeMember: vi.fn(),
  },
}));

describe('DashboardLayout share modal', () => {
  const useAppStoreMock = vi.mocked(useAppStore);
  const createShareMock = vi.mocked(kbApi.createShare);
  const getShareSettingsMock = vi.mocked(kbApi.getShareSettings);
  const writeTextMock = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();

    vi.stubGlobal('location', {
      href: 'http://localhost:3000/dashboard',
      pathname: '/dashboard',
      origin: 'http://localhost:3000',
    });

    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: writeTextMock,
      },
    });
    writeTextMock.mockResolvedValue(undefined);

    useAppStoreMock.mockReturnValue({
      currentWorkspace: {
        id: 'kb-1',
        title: 'Workspace A',
      },
      toggleSearchDialog: vi.fn(),
      user: {
        id: 'user-1',
        email: 'u@example.com',
        full_name: 'User One',
      },
      setUser: vi.fn(),
      clearUser: vi.fn(),
      clearCurrentWorkspace: vi.fn(),
    });

    createShareMock.mockResolvedValue({
      success: true,
      data: {
        share_token: 'token-xyz',
        share_url: '/shared/kb/token-xyz',
      },
    });
    getShareSettingsMock.mockResolvedValue({
      success: true,
      data: {
        access_level: 'private',
        share_tokens: [],
        members: [],
      },
    });
  });

  it('auto-generates share link on open and keeps modal open after copy', async () => {
    const user = userEvent.setup();

    render(
      <DashboardLayout>
        <div>Child Content</div>
      </DashboardLayout>
    );

    const shareButton = await screen.findByTitle('dashboard.share');
    await user.click(shareButton);

    await screen.findAllByText('share.title');

    await waitFor(() => {
      expect(createShareMock).toHaveBeenCalledWith('kb-1', {
        permission: 'partial',
        expire_in_hours: 24,
      });
    });

    await screen.findByDisplayValue('http://localhost:3000/shared/kb/token-xyz');

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'share.copyLink' }).hasAttribute('disabled')).toBe(false);
    });

    await user.click(screen.getByRole('button', { name: 'share.copyLink' }));

    expect(screen.getAllByText('share.title').length).toBeGreaterThan(0);
  });
});
