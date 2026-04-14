// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { APIAccessModal } from './api-access-modal';
import { kbApi } from '@/lib/api/client';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      key === 'apiAccess.rateLimitValue' && options?.rpm
        ? `${options.rpm} RPM`
        : key,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/lib/api/client', () => ({
  kbApi: {
    listAPIKeys: vi.fn(),
    createAPIKey: vi.fn(),
    revokeAPIKey: vi.fn(),
  },
}));

describe('APIAccessModal', () => {
  const listAPIKeysMock = vi.mocked(kbApi.listAPIKeys);
  const createAPIKeyMock = vi.mocked(kbApi.createAPIKey);
  const revokeAPIKeyMock = vi.mocked(kbApi.revokeAPIKey);
  const writeTextMock = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('location', {
      origin: 'http://localhost:3000',
    });
    vi.stubGlobal('navigator', {
      clipboard: {
        writeText: writeTextMock,
      },
    });
    writeTextMock.mockResolvedValue(undefined);
    listAPIKeysMock.mockResolvedValue({
      success: true,
      data: [
        {
          id: 'key-1',
          org_id: 'org-1',
          notebook_id: 'kb-1',
          key_prefix: 'sk_12345678',
          name: 'Existing key',
          permissions: ['query'],
          rate_limit_rpm: 60,
          is_active: true,
          created_by: 'user-1',
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });
    createAPIKeyMock.mockResolvedValue({
      success: true,
      data: {
        api_key: {
          id: 'key-2',
          org_id: 'org-1',
          notebook_id: 'kb-1',
          key_prefix: 'sk_abcdef12',
          name: 'Query key',
          permissions: ['query'],
          rate_limit_rpm: 120,
          is_active: true,
          created_by: 'user-1',
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
        plaintext_key: 'sk_plaintext_once',
      },
    });
    revokeAPIKeyMock.mockResolvedValue({
      success: true,
      data: {
        status: 'revoked',
      },
    });
  });

  it('loads, creates, and revokes notebook api keys', async () => {
    const user = userEvent.setup();

    render(
      <APIAccessModal
        open
        workspace={{
          id: 'kb-1',
          user_id: 'user-1',
          title: 'Workspace A',
          created_at: new Date().toISOString(),
        }}
        onOpenChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(listAPIKeysMock).toHaveBeenCalledWith('kb-1');
    });

    expect(await screen.findByText('Existing key')).toBeTruthy();

    await user.type(screen.getByPlaceholderText('apiAccess.keyNamePlaceholder'), 'Query key');
    fireEvent.change(screen.getByLabelText('apiAccess.rateLimit'), {
      target: { value: '120' },
    });
    await user.click(screen.getAllByRole('button', { name: 'apiAccess.createKey' })[0]);

    await waitFor(() => {
      expect(createAPIKeyMock).toHaveBeenCalledWith('kb-1', {
        name: 'Query key',
        permissions: ['query'],
        rate_limit_rpm: 120,
      });
    });

    expect(await screen.findByText('apiAccess.plaintextKey')).toBeTruthy();

    await user.click(screen.getAllByRole('button', { name: 'apiAccess.revoke' })[0]);

    await waitFor(() => {
      expect(revokeAPIKeyMock).toHaveBeenCalledWith('kb-1', 'key-1');
    });
  });
});
