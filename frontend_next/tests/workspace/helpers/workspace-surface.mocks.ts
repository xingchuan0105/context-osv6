import { vi } from "vitest";

vi.hoisted(() => {
  globalThis.__workspaceSurfaceHarnessMocks =
    globalThis.__mockProviders.createWorkspaceSurfaceMocks();
});

/**
 * Registers workspace surface vi.mock blocks. Import this module before
 * importing components under test.
 */
export function installWorkspaceSurfaceMocks() {
  return globalThis.__workspaceSurfaceHarnessMocks;
}

export const workspaceSurfaceMocks = globalThis.__workspaceSurfaceHarnessMocks;

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: globalThis.__workspaceSurfaceHarnessMocks.pushMock,
    replace: globalThis.__workspaceSurfaceHarnessMocks.replaceMock,
    prefetch: vi.fn(),
    refresh: vi.fn(),
    back: vi.fn(),
    forward: vi.fn(),
  }),
}));

vi.mock("../../../lib/auth/context", () => ({
  useAuth: () => globalThis.__workspaceSurfaceHarnessMocks.authState,
}));

vi.mock("../../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: globalThis.__workspaceSurfaceHarnessMocks.setLocaleMock,
    setTheme: globalThis.__workspaceSurfaceHarnessMocks.setThemeMock,
  }),
}));

vi.mock("../../../lib/dashboard/client", () => ({
  createWorkspace: globalThis.__workspaceSurfaceHarnessMocks.createWorkspaceMock,
}));

vi.mock("../../../lib/dashboard/default-title", () => ({
  getDefaultWorkspaceTitle: globalThis.__workspaceSurfaceHarnessMocks.getDefaultWorkspaceTitleMock,
  markDefaultWorkspaceTitleUsed: globalThis.__workspaceSurfaceHarnessMocks.markDefaultWorkspaceTitleUsedMock,
}));

vi.mock("../../../lib/workspace/client", () => ({
  getWorkspace: globalThis.__workspaceSurfaceHarnessMocks.getWorkspaceMock,
  listWorkspaceSessions: globalThis.__workspaceSurfaceHarnessMocks.listWorkspaceSessionsMock,
  listWorkspaceSessionMessages: globalThis.__workspaceSurfaceHarnessMocks.listWorkspaceSessionMessagesMock,
  createWorkspaceSession: globalThis.__workspaceSurfaceHarnessMocks.createWorkspaceSessionMock,
  updateWorkspace: globalThis.__workspaceSurfaceHarnessMocks.updateWorkspaceMock,
  updateWorkspaceSession: globalThis.__workspaceSurfaceHarnessMocks.updateWorkspaceSessionMock,
  deleteWorkspaceSession: globalThis.__workspaceSurfaceHarnessMocks.deleteWorkspaceSessionMock,
  lookupWorkspaceCitation: globalThis.__workspaceSurfaceHarnessMocks.lookupWorkspaceCitationMock,
  addWorkspaceSourceUrl: globalThis.__workspaceSurfaceHarnessMocks.addWorkspaceSourceUrlMock,
  completeWorkspaceDocumentUpload: globalThis.__workspaceSurfaceHarnessMocks.completeWorkspaceDocumentUploadMock,
  createWorkspaceNote: globalThis.__workspaceSurfaceHarnessMocks.createWorkspaceNoteMock,
  createWorkspaceDocumentUpload: globalThis.__workspaceSurfaceHarnessMocks.createWorkspaceDocumentUploadMock,
  deleteWorkspaceDocument: globalThis.__workspaceSurfaceHarnessMocks.deleteWorkspaceDocumentMock,
  deleteWorkspaceNote: globalThis.__workspaceSurfaceHarnessMocks.deleteWorkspaceNoteMock,
  getWorkspaceSourceContent: globalThis.__workspaceSurfaceHarnessMocks.getWorkspaceSourceContentMock,
  getWorkspaceSourceParsedPreview: globalThis.__workspaceSurfaceHarnessMocks.getWorkspaceSourceParsedPreviewMock,
  listWorkspaceNotes: globalThis.__workspaceSurfaceHarnessMocks.listWorkspaceNotesMock,
  listWorkspaceSources: globalThis.__workspaceSurfaceHarnessMocks.listWorkspaceSourcesMock,
  promoteWorkspaceNote: globalThis.__workspaceSurfaceHarnessMocks.promoteWorkspaceNoteMock,
  reindexWorkspaceDocument: globalThis.__workspaceSurfaceHarnessMocks.reindexWorkspaceDocumentMock,
  uploadWorkspaceDocumentFile: globalThis.__workspaceSurfaceHarnessMocks.uploadWorkspaceDocumentFileMock,
  updateWorkspaceNote: globalThis.__workspaceSurfaceHarnessMocks.updateWorkspaceNoteMock,
}));

vi.mock("../../../lib/runtime/transport", () => ({
  streamChat: globalThis.__workspaceSurfaceHarnessMocks.streamWorkspaceChatMock,
}));

vi.mock("../../../lib/billing/featureFlag", () => ({
  isPricingRevampEnabledSSR: () => true,
  isPricingRevampEnabled: vi.fn().mockResolvedValue(true),
  probePricingRevampUsageWindow: globalThis.__workspaceSurfaceHarnessMocks.probePricingRevampUsageWindowMock,
}));
