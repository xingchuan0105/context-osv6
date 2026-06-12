import { vi } from "vitest";
import type { createWorkspaceRightRailMocks } from "../../helpers/mock-providers";
import { workspaceUiStore } from "../../../lib/workspace/ui-store";

export function resetWorkspaceRightRailMocks(
  mocks: ReturnType<typeof createWorkspaceRightRailMocks>,
) {
  workspaceUiStore.setState((state) => ({ ...state, workspaces: {} }));
  mocks.authState = {
    initialized: true,
    isAuthenticated: true,
    token: "token-123",
    user: { id: "user-1", email: "user@example.test", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  };
  mocks.uiPreferencesState = {
    locale: "en",
    theme: "system",
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  };

  mocks.addWorkspaceSourceUrlMock.mockReset();
  mocks.completeWorkspaceDocumentUploadMock.mockReset();
  mocks.createWorkspaceNoteMock.mockReset();
  mocks.createWorkspaceDocumentUploadMock.mockReset();
  mocks.deleteWorkspaceDocumentMock.mockReset();
  mocks.deleteWorkspaceNoteMock.mockReset();
  mocks.getWorkspaceSourceContentMock.mockReset();
  mocks.getWorkspaceSourceParsedPreviewMock.mockReset();
  mocks.listWorkspaceNotesMock.mockReset();
  mocks.listWorkspaceSourcesMock.mockReset();
  mocks.lookupWorkspaceCitationMock.mockReset();
  mocks.promoteWorkspaceNoteMock.mockReset();
  mocks.reindexWorkspaceDocumentMock.mockReset();
  mocks.uploadWorkspaceDocumentFileMock.mockReset();
  mocks.updateWorkspaceNoteMock.mockReset();
  mocks.getWorkspaceSourceContentMock.mockResolvedValue({
    content: "Fallback content",
    summary: null,
  });
  mocks.getWorkspaceSourceParsedPreviewMock.mockResolvedValue({
    items: [],
    has_more: false,
    next_cursor: 0,
    summary: null,
  });
  mocks.lookupWorkspaceCitationMock.mockResolvedValue({
    doc_name: "Doc",
    content: "Matched excerpt",
    doc_id: "src-1",
    chunk_id: "chunk-1",
    page: 2,
    chunk_type: "text",
    asset_id: null,
    caption: null,
    image_url: null,
  });
  window.localStorage.clear();
}
