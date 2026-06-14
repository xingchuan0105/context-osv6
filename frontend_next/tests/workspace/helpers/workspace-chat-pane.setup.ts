import { vi } from "vitest";
import type { createWorkspaceChatPaneMocks } from "../../helpers/mock-providers";
import { queryLibraryStore } from "../../../lib/workspace/query-library/store";
import { workspaceUiStore } from "../../../lib/workspace/ui-store";

export function mockReducedMotionPreference(matches: boolean) {
  const originalMatchMedia = window.matchMedia;

  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn((query: string) => ({
      matches: matches && query === "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }) as MediaQueryList),
  });

  return () => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      writable: true,
      value: originalMatchMedia,
    });
  };
}

export function resetWorkspaceChatPaneMocks(
  mocks: ReturnType<typeof createWorkspaceChatPaneMocks>,
) {
  window.localStorage.clear();
  workspaceUiStore.setState((state) => ({ ...state, workspaces: {} }));
  queryLibraryStore.setState({ workspaces: {} });
  mocks.listWorkspaceSessionMessagesMock.mockReset();
  mocks.streamWorkspaceChatMock.mockReset();
  mocks.useAuthMock.mockReset();
  mocks.useAuthMock.mockReturnValue({
    initialized: true,
    isAuthenticated: true,
    token: "token-123",
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  });
}
