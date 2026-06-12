import { vi } from "vitest";

export function createMockAuthState(overrides?: Partial<{
  initialized: boolean;
  token: string;
  isAuthenticated: boolean;
  user: { id: string; email: string; full_name: string };
  passwordResetEnabled: boolean;
}>) {
  return {
    initialized: true,
    token: "token-123",
    isAuthenticated: true,
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
    ...overrides,
  };
}

export function createRouterMocks() {
  return {
    pushMock: vi.fn(),
    replaceMock: vi.fn(),
  };
}

export function createUiPreferencesMocks() {
  return {
    setLocaleMock: vi.fn(),
    setThemeMock: vi.fn(),
  };
}

export function createWorkspaceMocks() {
  return {
    getWorkspaceMock: vi.fn(),
    listWorkspaceSessionsMock: vi.fn(),
    listWorkspaceSessionMessagesMock: vi.fn(),
    createWorkspaceSessionMock: vi.fn(),
    updateWorkspaceMock: vi.fn(),
    updateWorkspaceSessionMock: vi.fn(),
    deleteWorkspaceSessionMock: vi.fn(),
    lookupWorkspaceCitationMock: vi.fn(),
  };
}

export function createDashboardMocks() {
  return {
    createWorkspaceMock: vi.fn(),
    getDefaultWorkspaceTitleMock: vi.fn(),
    markDefaultWorkspaceTitleUsedMock: vi.fn(),
  };
}

export function createBillingMocks() {
  return {
    getUsageWindowMock: vi.fn(),
  };
}
