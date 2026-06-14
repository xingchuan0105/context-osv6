import { vi } from "vitest";

const queryLibraryCaptureMock = vi.hoisted(() => vi.fn());

vi.hoisted(() => {
  globalThis.__workspaceChatPaneHarnessMocks =
    globalThis.__mockProviders.createWorkspaceChatPaneMocks();
});

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => globalThis.__workspaceChatPaneHarnessMocks.useAuthMock(),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/workspace/client", () => ({
  listWorkspaceSessionMessages: globalThis.__workspaceChatPaneHarnessMocks.listWorkspaceSessionMessagesMock,
}));

vi.mock("../../lib/runtime/transport", () => ({
  streamChat: globalThis.__workspaceChatPaneHarnessMocks.streamWorkspaceChatMock,
}));

vi.mock("../../lib/workspace/query-library/store", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/workspace/query-library/store")>();
  const originalGetState = actual.queryLibraryStore.getState.bind(actual.queryLibraryStore);

  const queryLibraryStore = Object.assign(actual.queryLibraryStore, {
    getState: () => ({
      ...originalGetState(),
      capture: queryLibraryCaptureMock,
    }),
  });

  return {
    ...actual,
    queryLibraryStore,
  };
});

export const workspaceChatPaneMocks = globalThis.__workspaceChatPaneHarnessMocks;
export const queryLibraryCaptureHarnessMock = queryLibraryCaptureMock;
