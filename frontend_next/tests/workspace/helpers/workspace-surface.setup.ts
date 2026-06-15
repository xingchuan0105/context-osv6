import { vi } from "vitest";
import type { createWorkspaceSurfaceMocks } from "../../helpers/mock-providers";
import { workspaceUiStore } from "../../../lib/workspace/ui-store";

let mobileViewport = false;
const matchMediaListeners = new Set<(event: MediaQueryListEvent) => void>();

export function setMobileViewport(next: boolean) {
  mobileViewport = next;
  for (const listener of matchMediaListeners) {
    listener({ matches: next } as MediaQueryListEvent);
  }
}

export function installSurfaceMatchMedia() {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn().mockImplementation(() => ({
      matches: mobileViewport,
      media: "(max-width: 767px)",
      onchange: null,
      addEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.add(listener);
      },
      removeEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.delete(listener);
      },
      addListener: (listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.add(listener);
      },
      removeListener: (listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.delete(listener);
      },
      dispatchEvent: vi.fn(),
    })),
  });
}

export function clearSurfaceMatchMediaListeners() {
  matchMediaListeners.clear();
}

export function buildWorkspaceSource(
  overrides: Partial<{ id: string; file_name: string; status: string; title: string }> = {},
) {
  return {
    id: overrides.id ?? "src-1",
    workspace_id: "ws-1",
    workspace_name: "Workspace 1",
    title: overrides.title ?? "Source",
    file_name: overrides.file_name ?? "source.pdf",
    status: overrides.status ?? "ready",
  };
}

export function resetWorkspaceSurfaceMocks(
  mocks: ReturnType<typeof createWorkspaceSurfaceMocks>,
) {
  setMobileViewport(false);
  window.localStorage.clear();
  workspaceUiStore.setState((state) => ({ ...state, workspaces: {} }));

  mocks.pushMock.mockReset();
  mocks.replaceMock.mockReset();
  mocks.createWorkspaceMock.mockReset();
  mocks.getDefaultWorkspaceTitleMock.mockReset();
  mocks.markDefaultWorkspaceTitleUsedMock.mockReset();
  mocks.setLocaleMock.mockReset();
  mocks.setThemeMock.mockReset();
  mocks.logoutMock.mockReset();
  mocks.getWorkspaceMock.mockReset();
  mocks.listWorkspaceSessionsMock.mockReset();
  mocks.listWorkspaceSessionMessagesMock.mockReset();
  mocks.createWorkspaceSessionMock.mockReset();
  mocks.updateWorkspaceMock.mockReset();
  mocks.updateWorkspaceSessionMock.mockReset();
  mocks.deleteWorkspaceSessionMock.mockReset();
  mocks.lookupWorkspaceCitationMock.mockReset();
  mocks.streamWorkspaceChatMock.mockReset();
  mocks.getUsageWindowMock.mockReset();
  mocks.probePricingRevampUsageWindowMock.mockReset();
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
  mocks.promoteWorkspaceNoteMock.mockReset();
  mocks.reindexWorkspaceDocumentMock.mockReset();
  mocks.uploadWorkspaceDocumentFileMock.mockReset();
  mocks.updateWorkspaceNoteMock.mockReset();

  mocks.getUsageWindowMock.mockResolvedValue({
    plan_id: "free",
    rolling_5h: { used: 1000, limit: 100000, percentage: 1, reset_at: "2099-01-01T00:00:00Z" },
    rolling_7d: { used: 5000, limit: 400000, percentage: 1, reset_at: "2099-01-01T00:00:00Z" },
    soft_limit_hit: { rolling_5h: false, rolling_7d: false },
    hard_limit_hit: { rolling_5h: false, rolling_7d: false },
  });
  mocks.probePricingRevampUsageWindowMock.mockImplementation(async () => ({
    enabled: true,
    usageWindow: await mocks.getUsageWindowMock(),
  }));

  mocks.getWorkspaceMock.mockResolvedValue({
    workspace: {
      workspace_id: "ws-1",
      org_id: "org-1",
      owner_id: "user-1",
      name: "Workspace 1",
      title: "Workspace 1",
      description: "A workspace",
      created_at: "2026-04-17T00:00:00Z",
      updated_at: "2026-04-18T00:00:00Z",
      document_count: 2,
      status_summary: { ready: 1 },
      shared: false,
    },
  });
  mocks.listWorkspaceSessionsMock.mockResolvedValue({
    sessions: [
      {
        id: "sess-1",
        workspace_id: "ws-1",
        title: "Pinned thread",
        agent_type: "rag",
        pinned: true,
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-18T00:00:00Z",
      },
      {
        id: "sess-2",
        workspace_id: "ws-1",
        title: "General thread",
        agent_type: "rag",
        pinned: false,
        created_at: "2026-04-16T00:00:00Z",
        updated_at: "2026-04-17T00:00:00Z",
      },
    ],
  });
  mocks.createWorkspaceSessionMock.mockResolvedValue({
    id: "sess-3",
    workspace_id: "ws-1",
    title: "New thread",
    agent_type: "rag",
    pinned: false,
    created_at: "2026-04-19T00:00:00Z",
    updated_at: "2026-04-19T00:00:00Z",
  });
  mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
  mocks.updateWorkspaceSessionMock.mockImplementation(
    async (_token, sessionId, requestBody: { title?: string | null; pinned?: boolean | null }) => {
      if (requestBody.pinned !== undefined) {
        return {
          id: sessionId,
          workspace_id: "ws-1",
          title: "General thread",
          agent_type: "rag",
          pinned: Boolean(requestBody.pinned),
          created_at: "2026-04-16T00:00:00Z",
          updated_at: "2026-04-19T00:00:00Z",
        };
      }

      return {
        id: sessionId,
        workspace_id: "ws-1",
        title: requestBody.title ?? null,
        agent_type: "rag",
        pinned: false,
        created_at: "2026-04-16T00:00:00Z",
        updated_at: "2026-04-19T00:00:00Z",
      };
    },
  );
  mocks.updateWorkspaceMock.mockResolvedValue({
    workspace: {
      workspace_id: "ws-1",
      org_id: "org-1",
      owner_id: "user-1",
      name: "Workspace 1",
      title: "Renamed Workspace",
      description: "A workspace",
      created_at: "2026-04-17T00:00:00Z",
      updated_at: "2026-04-19T00:00:00Z",
      document_count: 2,
      status_summary: { ready: 1 },
      shared: false,
    },
  });
  mocks.deleteWorkspaceSessionMock.mockResolvedValue(undefined);
  mocks.lookupWorkspaceCitationMock.mockResolvedValue({
    doc_name: "Source Two",
    content: "Chunk detail for src-2",
    doc_id: "src-2",
    chunk_id: "chunk-1",
    page: 3,
    chunk_type: "text",
    asset_id: null,
    caption: null,
    image_url: null,
  });
  mocks.createWorkspaceMock.mockResolvedValue({
    workspace: {
      workspace_id: "ws-2",
      org_id: "org-1",
      owner_id: "user-1",
      name: "工作区1",
      title: "工作区1",
      description: "",
      created_at: "2026-04-19T00:00:00Z",
      updated_at: "2026-04-19T00:00:00Z",
      document_count: 0,
      status_summary: {},
      shared: false,
    },
  });
  mocks.getDefaultWorkspaceTitleMock.mockReturnValue("工作区1");
  mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });
  mocks.listWorkspaceSourcesMock.mockResolvedValue({
    sources: [
      buildWorkspaceSource({ id: "src-1", file_name: "source-one.pdf", status: "ready", title: "Source One" }),
      buildWorkspaceSource({ id: "src-2", file_name: "source-two.pdf", status: "ready", title: "Source Two" }),
    ],
  });
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
  mocks.authState = {
    initialized: true,
    token: "token-123",
    isAuthenticated: true,
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: mocks.logoutMock,
  };
}
