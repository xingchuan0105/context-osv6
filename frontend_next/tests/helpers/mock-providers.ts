import { vi } from "vitest";
import type { BillingPlan } from "../../lib/billing/api";

export function createMockAuthState(overrides?: Partial<{
  initialized: boolean;
  token: string | null;
  isAuthenticated: boolean;
  user: { id: string; email: string; full_name: string } | null;
  passwordResetEnabled: boolean;
  completeAuth: ReturnType<typeof vi.fn>;
  updateUser: ReturnType<typeof vi.fn>;
  clearAuth: ReturnType<typeof vi.fn>;
  logout: ReturnType<typeof vi.fn>;
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

export function createUiPreferencesState(overrides?: Partial<{
  locale: "zh-CN" | "en";
  theme: "system" | "light" | "dark";
  setLocale: ReturnType<typeof vi.fn>;
  setTheme: ReturnType<typeof vi.fn>;
}>) {
  return {
    locale: "en" as "zh-CN" | "en",
    theme: "system" as "system" | "light" | "dark",
    setLocale: vi.fn(),
    setTheme: vi.fn(),
    ...overrides,
  };
}

export function createWorkspaceClientMocks() {
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

export function createComponentMock() {
  return vi.fn();
}

export function createWorkspaceSurfaceMocks() {
  const { pushMock, replaceMock } = createRouterMocks();
  const { setLocaleMock, setThemeMock } = createUiPreferencesMocks();
  const logoutMock = vi.fn();
  const rightRail = createWorkspaceRightRailMocks();

  return {
    pushMock,
    replaceMock,
    ...createDashboardMocks(),
    setLocaleMock,
    setThemeMock,
    logoutMock,
    authState: createMockAuthState(),
    ...createWorkspaceClientMocks(),
    streamWorkspaceChatMock: vi.fn(),
    getUsageWindowMock: vi.fn(),
    probePricingRevampUsageWindowMock: vi.fn(),
    addWorkspaceSourceUrlMock: rightRail.addWorkspaceSourceUrlMock,
    completeWorkspaceDocumentUploadMock: rightRail.completeWorkspaceDocumentUploadMock,
    createWorkspaceNoteMock: rightRail.createWorkspaceNoteMock,
    createWorkspaceDocumentUploadMock: rightRail.createWorkspaceDocumentUploadMock,
    deleteWorkspaceDocumentMock: rightRail.deleteWorkspaceDocumentMock,
    deleteWorkspaceNoteMock: rightRail.deleteWorkspaceNoteMock,
    getWorkspaceSourceContentMock: rightRail.getWorkspaceSourceContentMock,
    getWorkspaceSourceParsedPreviewMock: rightRail.getWorkspaceSourceParsedPreviewMock,
    listWorkspaceNotesMock: rightRail.listWorkspaceNotesMock,
    listWorkspaceSourcesMock: rightRail.listWorkspaceSourcesMock,
    promoteWorkspaceNoteMock: rightRail.promoteWorkspaceNoteMock,
    reindexWorkspaceDocumentMock: rightRail.reindexWorkspaceDocumentMock,
    uploadWorkspaceDocumentFileMock: rightRail.uploadWorkspaceDocumentFileMock,
    updateWorkspaceNoteMock: rightRail.updateWorkspaceNoteMock,
  };
}

export function createDashboardSurfaceMocks() {
  const { pushMock } = createRouterMocks();

  return {
    pushMock,
    listWorkspacesMock: vi.fn(),
    getFavoriteWorkspaceIdsMock: vi.fn(),
    createWorkspaceMock: vi.fn(),
    updateWorkspaceMock: vi.fn(),
    deleteWorkspaceMock: vi.fn(),
    updateFavoriteWorkspaceIdsMock: vi.fn(),
    getUsageLimitMock: vi.fn(),
    authState: createMockAuthState(),
  };
}

export function createSettingsSurfaceMocks() {
  const { replaceMock } = createRouterMocks();
  const { setLocaleMock, setThemeMock } = createUiPreferencesMocks();

  return {
    replaceMock,
    changePasswordMock: vi.fn(),
    clearAuthMock: vi.fn(),
    logoutMock: vi.fn(),
    updateUserMock: vi.fn(),
    createPortalSessionMock: vi.fn(),
    getSubscriptionMock: vi.fn(),
    getUsageMock: vi.fn(),
    getUsageLimitMock: vi.fn(),
    getUserPreferencesMock: vi.fn(),
    listNotificationsMock: vi.fn(),
    listPlansMock: vi.fn(),
    markNotificationReadMock: vi.fn(),
    updateProfileMock: vi.fn(),
    updateUserPreferencesMock: vi.fn(),
    setLocaleMock,
    setThemeMock,
    authState: {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner",
      },
      clearAuth: vi.fn(),
      updateUser: vi.fn(),
      logout: vi.fn(),
      passwordResetEnabled: true,
    } as {
      token: string | null;
      user: {
        id: string;
        email: string;
        full_name: string;
      } | null;
      clearAuth: () => void;
      updateUser: (user: { id: string; email: string; full_name: string }) => void;
      logout: () => Promise<void>;
      passwordResetEnabled: boolean;
    },
    uiPreferencesState: createUiPreferencesState(),
  };
}

export function createWorkspaceShareSurfaceMocks() {
  return {
    authState: {
      token: "token-123",
    },
    uiPreferencesState: {
      locale: "en" as "zh-CN" | "en",
    },
    buildShareUrlMock: vi.fn(),
    getShareSettingsMock: vi.fn(),
    listMembersMock: vi.fn(),
    updateShareSettingsMock: vi.fn(),
    createShareLinkMock: vi.fn(),
    revokeShareLinkMock: vi.fn(),
    inviteMemberMock: vi.fn(),
    getShareAnalyticsMock: vi.fn(),
    getShareAccessLogsMock: vi.fn(),
    removeMemberMock: vi.fn(),
  };
}

export function createSharedWorkspaceSurfaceMocks() {
  return {
    authState: {
      initialized: true,
      token: "token-123" as string | null,
    },
    getSharedWorkspaceMock: vi.fn(),
    streamSharedChatMock: vi.fn(),
  };
}

export function createWorkspaceRightRailMocks() {
  return {
    authState: createMockAuthState({
      user: { id: "user-1", email: "user@example.test", full_name: "User Example" },
    }),
    uiPreferencesState: createUiPreferencesState({ locale: "en" }),
    addWorkspaceSourceUrlMock: vi.fn(),
    completeWorkspaceDocumentUploadMock: vi.fn(),
    createWorkspaceNoteMock: vi.fn(),
    createWorkspaceDocumentUploadMock: vi.fn(),
    deleteWorkspaceDocumentMock: vi.fn(),
    deleteWorkspaceNoteMock: vi.fn(),
    getWorkspaceSourceContentMock: vi.fn(),
    getWorkspaceSourceParsedPreviewMock: vi.fn(),
    listWorkspaceNotesMock: vi.fn(),
    listWorkspaceSourcesMock: vi.fn(),
    lookupWorkspaceCitationMock: vi.fn(),
    promoteWorkspaceNoteMock: vi.fn(),
    reindexWorkspaceDocumentMock: vi.fn(),
    uploadWorkspaceDocumentFileMock: vi.fn(),
    updateWorkspaceNoteMock: vi.fn(),
  };
}

export function createWorkspaceApiAccessSurfaceMocks() {
  return {
    authState: {
      token: "token-123",
    },
    getApiAccessBaseUrlMock: vi.fn(),
    listApiKeysMock: vi.fn(),
    createApiKeyMock: vi.fn(),
    revokeApiKeyMock: vi.fn(),
  };
}

export function createWorkspaceHistoryPaneMocks() {
  return {
    listWorkspaceSessionMessagesMock: vi.fn(),
  };
}

export function createWorkspaceChatPaneMocks() {
  return {
    listWorkspaceSessionMessagesMock: vi.fn(),
    streamWorkspaceChatMock: vi.fn(),
    useAuthMock: vi.fn(),
  };
}

export function createAdminSurfacesMocks() {
  return {
    authState: {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner",
      },
    },
    uiPreferencesState: {
      locale: "en" as "zh-CN" | "en",
    },
    listAdminOrganizationsMock: vi.fn(),
    getAdminOrganizationMock: vi.fn(),
    listAdminUsersForOrganizationMock: vi.fn(),
    getAdminUsageForOrganizationMock: vi.fn(),
    updateAdminOrganizationBlockedMock: vi.fn(),
    getAdminHealthMock: vi.fn(),
    getAdminBillingOverviewMock: vi.fn(),
    getAdminRagHealthMock: vi.fn(),
    listAdminFeatureFlagsMock: vi.fn(),
    requestAdminFeatureFlagChangeMock: vi.fn(),
    reviewAdminFeatureFlagChangeMock: vi.fn(),
    listAdminFeatureFlagChangeRequestsMock: vi.fn(),
    getAdminWorkerStatusMock: vi.fn(),
    getAdminDegradationStatusMock: vi.fn(),
    listAdminAuditLogsMock: vi.fn(),
    exportAdminAuditLogsCsvMock: vi.fn(),
  };
}

export function createAuthContextMocks() {
  return {
    meMock: vi.fn(),
    logoutMock: vi.fn(),
    authRuntimeCapabilitiesMock: vi.fn(),
  };
}

export function createLoginRegisterMocks() {
  return {
    completeAuthMock: vi.fn(),
    loginMock: vi.fn(),
    registerMock: vi.fn(),
    replaceMock: vi.fn(),
    useRouterMock: vi.fn(),
    useSearchParamsMock: vi.fn(),
    useAuthMock: vi.fn(),
  };
}

export function createResetPasswordFlowMocks() {
  return {
    replaceMock: vi.fn(),
    useAuthMock: vi.fn(),
    useRouterMock: vi.fn(),
    sendResetCodeMock: vi.fn(),
    verifyResetCodeMock: vi.fn(),
    confirmResetPasswordMock: vi.fn(),
  };
}

export function createInviteSurfaceMocks() {
  const { pushMock } = createRouterMocks();

  return {
    pushMock,
    authState: {
      initialized: true,
      token: null as string | null,
    },
    getWorkspaceMock: vi.fn(),
    acceptInviteMock: vi.fn(),
    declineInviteMock: vi.fn(),
  };
}

export function createUsagePageMocks() {
  return {
    ...createRouterMocks(),
    isPricingRevampEnabledMock: vi.fn(),
  };
}

export function createPricingPageMockPlans(): BillingPlan[] {
  return [
    {
      plan_id: "free",
      name: "Free",
      price_label_cny: "¥0",
      price_label_usd: "$0",
      description: "体验",
      price_label: "¥0",
      interval: "month",
      checkout_available: false,
      current: false,
      quotas: [],
    },
    {
      plan_id: "plus",
      name: "Plus",
      price_label_cny: "¥49 / 月",
      price_label_usd: "$9 / 月",
      description: "深度研究",
      price_label: "¥49 / 月 · $9 / 月",
      interval: "month",
      checkout_available: true,
      current: false,
      quotas: [],
    },
    {
      plan_id: "pro",
      name: "Pro",
      price_label_cny: "¥129 / 月",
      price_label_usd: "$19 / 月",
      description: "重度无忧",
      price_label: "¥129 / 月 · $19 / 月",
      interval: "month",
      checkout_available: true,
      current: false,
      quotas: [],
    },
  ];
}
