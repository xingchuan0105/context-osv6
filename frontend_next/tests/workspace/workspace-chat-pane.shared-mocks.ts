import { vi } from "vitest";

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

export const workspaceChatPaneMocks = globalThis.__workspaceChatPaneHarnessMocks;
