import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DraftList } from './draft-list';
import { useAppStore } from '@/stores/useAppStore';
import { documentsApi, notesApi } from '@/lib/api/client';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: {
      resolvedLanguage: 'zh-CN',
    },
  }),
}));

vi.mock('@/stores/useAppStore', () => ({
  useAppStore: vi.fn(),
}));

vi.mock('@/lib/api/client', () => ({
  notesApi: {
    list: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
  },
  documentsApi: {
    upload: vi.fn(),
  },
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('./note-editor', () => ({
  NoteEditor: () => <div data-testid="note-editor" />,
}));

describe('DraftList', () => {
  const useAppStoreMock = vi.mocked(useAppStore);
  const listNotesMock = vi.mocked(notesApi.list);
  const uploadSourceMock = vi.mocked(documentsApi.upload);

  beforeEach(() => {
    vi.clearAllMocks();

    useAppStoreMock.mockReturnValue({
      currentWorkspace: {
        id: 'kb-1',
        title: 'Workspace A',
      },
    });

    listNotesMock.mockResolvedValue({
      success: true,
      data: [
        {
          id: 'note-1',
          kb_id: 'kb-1',
          user_id: 'user-1',
          title: 'Meeting Notes',
          content: 'Action items for next sprint',
          note_type: 'draft',
          is_shared: false,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-02T00:00:00Z',
        },
      ],
    });

    uploadSourceMock.mockResolvedValue({
      success: true,
      data: {
        id: 'doc-1',
        document_id: 'doc-1',
        status: 'queued',
      },
    });
  });

  it('imports a note as content source from notes list', async () => {
    const user = userEvent.setup();
    render(<DraftList />);

    await screen.findByText('Meeting Notes');

    await user.click(screen.getByRole('button', { name: 'note.importToKB' }));

    await waitFor(() => {
      expect(uploadSourceMock).toHaveBeenCalledTimes(1);
    });

    const [kbId, file] = uploadSourceMock.mock.calls[0] as [string, File];
    expect(kbId).toBe('kb-1');
    expect(file).toBeInstanceOf(File);
    expect(file.name).toBe('Meeting Notes.md');
  });
});
