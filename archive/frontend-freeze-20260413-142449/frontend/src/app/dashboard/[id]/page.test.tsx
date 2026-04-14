import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import WorkspaceDetailPage from './page';
import { useAppStore } from '@/stores/useAppStore';
import { documentsApi, notesApi, kbApi } from '@/lib/api/client';

const mockI18n = {
  resolvedLanguage: 'zh',
  language: 'zh',
};
const mockT = vi.fn((key: string) => key);
const mockPush = vi.fn();

// Mock dependencies
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: mockT,
    i18n: mockI18n,
  }),
}));

vi.mock('next/navigation', () => ({
  useParams: () => ({ id: 'kb-1' }),
  useRouter: () => ({ push: mockPush }),
}));

vi.mock('@/stores/useAppStore', () => ({
  useAppStore: vi.fn(),
}));

vi.mock('@/lib/api/client', () => ({
  authApi: { me: vi.fn() },
  clearAuthToken: vi.fn(),
  getAuthToken: vi.fn(() => 'test-token'),
  documentsApi: {
    list: vi.fn(),
    upload: vi.fn(),
    delete: vi.fn(),
    update: vi.fn(),
    listStatusEvents: vi.fn(),
    streamStatusEvents: vi.fn(),
  },
  notesApi: {
    list: vi.fn(),
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
  },
  kbApi: {
    list: vi.fn(),
    get: vi.fn(),
    delete: vi.fn(),
  },
}));

vi.mock('@/components/chat/chat-panel', () => ({
  ChatPanel: () => <div data-testid="chat-panel">Chat Panel</div>,
}));

vi.mock('@/components/dashboard/add-source-modal', () => ({
  AddSourceModal: ({ open }: { open: boolean }) =>
    open ? <div data-testid="add-source-modal">Add Source Modal</div> : null,
}));

vi.mock('@/components/dashboard/create-note-modal', () => ({
  CreateNoteModal: ({ open }: { open: boolean }) =>
    open ? <div data-testid="create-note-modal">Create Note Modal</div> : null,
}));

vi.mock('@/components/document/document-viewer', () => ({
  DocumentViewer: () => <div data-testid="document-viewer">Document Viewer</div>,
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

describe('WorkspaceDetailPage - T1-T4 Static UI Tests', () => {
  const useAppStoreMock = vi.mocked(useAppStore);
  const documentsListMock = vi.mocked(documentsApi.list);
  const notesListMock = vi.mocked(notesApi.list);
  const kbGetMock = vi.mocked(kbApi.get);
  const listStatusEventsMock = vi.mocked(documentsApi.listStatusEvents);
  const streamStatusEventsMock = vi.mocked(documentsApi.streamStatusEvents);

  beforeEach(() => {
    vi.clearAllMocks();

    useAppStoreMock.mockReturnValue({
      currentWorkspace: { id: 'kb-1', title: 'Test Workspace' },
      setCurrentWorkspace: vi.fn(),
      clearCurrentWorkspace: vi.fn(),
    } as any);

    kbGetMock.mockResolvedValue({
      success: true,
      data: {
        id: 'kb-1',
        user_id: '00000000-0000-0000-0000-000000000001',
        title: 'Test Workspace',
        description: '',
        created_at: '2026-01-01T00:00:00Z',
      },
    });

    documentsListMock.mockResolvedValue({
      success: true,
      data: [],
    });

    notesListMock.mockResolvedValue({
      success: true,
      data: [],
    });

    listStatusEventsMock.mockResolvedValue({
      success: true,
      data: { events: [] },
    });

    // Mock stream to immediately abort to prevent infinite loop
    streamStatusEventsMock.mockImplementation(async (_kbId, _seq, options) => {
      return new Promise((_, reject) => {
        if (options?.signal) {
          const abortHandler = () => reject(new Error('Aborted'));
          if (options.signal.aborted) {
            reject(new Error('Aborted'));
          } else {
            options.signal.addEventListener('abort', abortHandler, { once: true });
          }
        }
      });
    });
  });

  // T1: 左栏折叠窄轨形态
  it('T1: should render left panel with correct data attributes', async () => {
    render(<WorkspaceDetailPage />);

    const leftPanel = await screen.findByTestId('left-panel', {}, { timeout: 2000 });
    expect(leftPanel).toBeInTheDocument();
    expect(leftPanel).toHaveAttribute('data-collapsed', 'false');
  });

  // T2: 双栏拖拽范围修正
  it('T2: should render resize handle between panels', async () => {
    render(<WorkspaceDetailPage />);

    await screen.findByTestId('left-panel', {}, { timeout: 2000 });

    const resizeHandle = screen.getByTestId('resize-handle');
    expect(resizeHandle).toBeInTheDocument();
  });

  it('T2: should have default width of 30%', async () => {
    render(<WorkspaceDetailPage />);

    const leftPanel = await screen.findByTestId('left-panel', {}, { timeout: 2000 });
    expect(leftPanel).toHaveStyle({ width: '30%' });
  });

  // T3: 左栏样式改造
  it('T3: should have rounded corners and margin styling', async () => {
    render(<WorkspaceDetailPage />);

    const leftPanel = await screen.findByTestId('left-panel', {}, { timeout: 2000 });
    expect(leftPanel).toHaveClass('rounded-xl');
    expect(leftPanel).toHaveClass('m-2');
    expect(leftPanel).toHaveClass('bg-card');
  });

  // T4: 左栏按钮等宽与视觉统一
  it('T4: should render action buttons with correct titles', async () => {
    render(<WorkspaceDetailPage />);

    await screen.findByTestId('left-panel', {}, { timeout: 2000 });

    expect(screen.getByTitle('dashboard.addSource')).toBeInTheDocument();
    expect(screen.getByTitle('dashboard.createNote')).toBeInTheDocument();
  });
});
