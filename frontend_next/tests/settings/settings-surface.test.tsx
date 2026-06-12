import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    replace: mocks.replaceMock,
  }),
}));

vi.mock("../../lib/auth/client", () => ({
  changePassword: mocks.changePasswordMock,
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/settings/client", () => ({
  createPortalSession: mocks.createPortalSessionMock,
  defaultNotificationPreferences: () => ({
    email_enabled: true,
    product_enabled: true,
    security_enabled: true,
    weekly_digest_enabled: false,
    quiet_hours_start: null,
    quiet_hours_end: null,
  }),
  getSubscription: mocks.getSubscriptionMock,
  getUsage: mocks.getUsageMock,
  getUsageLimit: mocks.getUsageLimitMock,
  getUserPreferences: mocks.getUserPreferencesMock,
  listNotifications: mocks.listNotificationsMock,
  listPlans: mocks.listPlansMock,
  markNotificationRead: mocks.markNotificationReadMock,
  updateProfile: mocks.updateProfileMock,
  updateUserPreferences: mocks.updateUserPreferencesMock,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => mocks.uiPreferencesState,
}));

import {
  normalizeSettingsTab,
  type SettingsTab,
} from "../../components/settings/settings-tabs";
import { SettingsSurface } from "../../components/settings/settings-surface";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createSettingsSurfaceMocks());



function createTestQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
      mutations: {
        retry: false,
      },
    },
  });
}

function renderWithQuery(ui: ReactElement) {
  const queryClient = createTestQueryClient();

  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

function rerenderWithQuery(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
      mutations: {
        retry: false,
      },
    },
  });

  return <QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>;
}

describe("normalizeSettingsTab", () => {
  it.each([
    [undefined, "billing"],
    ["", "billing"],
    ["missing", "billing"],
    ["profile", "profile"],
    [["security"], "security"],
  ])("maps %s to %s", (value, expected) => {
    expect(normalizeSettingsTab(value as string | string[] | undefined)).toBe(expected);
  });
});

describe("SettingsSurface", () => {
  beforeEach(() => {
    mocks.replaceMock.mockReset();
    mocks.changePasswordMock.mockReset();
    mocks.clearAuthMock.mockReset();
    mocks.logoutMock.mockReset();
    mocks.updateUserMock.mockReset();
    mocks.createPortalSessionMock.mockReset();
    mocks.getSubscriptionMock.mockReset();
    mocks.getUsageMock.mockReset();
    mocks.getUsageLimitMock.mockReset();
    mocks.getUserPreferencesMock.mockReset();
    mocks.listNotificationsMock.mockReset();
    mocks.listPlansMock.mockReset();
    mocks.markNotificationReadMock.mockReset();
    mocks.updateProfileMock.mockReset();
    mocks.updateUserPreferencesMock.mockReset();
    mocks.setLocaleMock.mockReset();
    mocks.setThemeMock.mockReset();
    mocks.changePasswordMock.mockResolvedValue({
      success: true,
      data: null,
      error: null,
    });
    mocks.logoutMock.mockResolvedValue(undefined);
    mocks.createPortalSessionMock.mockResolvedValue({
      url: "https://billing.example.test",
    });
    mocks.getSubscriptionMock.mockResolvedValue({
      plan_id: "pro",
      status: "active",
      current_period_end: "2026-05-01T00:00:00Z",
    });
    mocks.getUsageMock.mockResolvedValue({
      used_tokens: 1500,
      limit_tokens: 0,
      used_documents: 12,
      limit_documents: 0,
    });
    mocks.getUsageLimitMock.mockResolvedValue({
      policy: {
        enabled: true,
        rolling_5h_limit_units: 1000,
        rolling_7d_limit_units: 7000,
      },
      windows: {
        rolling_5h: {
          used_units: 250,
          limit_units: 1000,
          remaining_units: 750,
          percent_used: 25,
          blocked: false,
          next_relief_at: "2026-04-20T12:00:00Z",
          blocked_until: null,
        },
        rolling_7d: {
          used_units: 1000,
          limit_units: 7000,
          remaining_units: 6000,
          percent_used: 14.3,
          blocked: false,
          next_relief_at: null,
          blocked_until: null,
        },
      },
      breakdown: {
        embedding_tokens: 300,
        llm_input_tokens: 400,
      },
      scope: {
        plan_default: {
          plan_id: "pro",
        },
      },
      has_estimated_usage: false,
    });
    mocks.getUserPreferencesMock.mockResolvedValue({
      dashboard: {
        favorite_notebook_ids: [],
        workspace_drafts: [],
        workspace_preferences: [],
        notebook_notes: [],
      },
      notifications: {
        email_enabled: true,
        product_enabled: true,
        security_enabled: true,
        weekly_digest_enabled: false,
        quiet_hours_start: null,
        quiet_hours_end: null,
      },
    });
    mocks.listNotificationsMock.mockResolvedValue({
      notifications: [
        {
          id: "notif-1",
          org_id: "org-1",
          user_id: "user-1",
          event_type: "security_alert",
          title: "Security alert",
          body: "A new sign-in device was detected.",
          data: {},
          read_at: null,
          created_at: "2026-04-20T10:00:00Z",
          updated_at: "2026-04-20T10:00:00Z",
        },
      ],
    });
    mocks.listPlansMock.mockResolvedValue({
      plans: [
        {
          id: "pro",
          name: "Pro",
          price: 2900,
          features: ["embedding_tokens: 100000", "pages_processed: 500"],
        },
      ],
    });
    mocks.markNotificationReadMock.mockResolvedValue({});
    mocks.updateProfileMock.mockResolvedValue({
      success: true,
      data: {
        token: "token-123",
        user: {
          id: "user-1",
          email: "owner@example.com",
          full_name: "Owner Updated",
        },
        reset_ticket: null,
      },
      error: null,
    });
    mocks.updateUserPreferencesMock.mockResolvedValue({
      dashboard: {
        favorite_notebook_ids: [],
        workspace_drafts: [],
        workspace_preferences: [],
        notebook_notes: [],
      },
      notifications: {
        email_enabled: true,
        product_enabled: true,
        security_enabled: false,
        weekly_digest_enabled: false,
        quiet_hours_start: null,
        quiet_hours_end: null,
      },
    });
    mocks.authState = {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner",
      },
      clearAuth: mocks.clearAuthMock,
      updateUser: mocks.updateUserMock,
      logout: mocks.logoutMock,
      passwordResetEnabled: true,
    };
    mocks.uiPreferencesState = {
      locale: "en",
      theme: "system",
      setLocale: mocks.setLocaleMock,
      setTheme: mocks.setThemeMock,
    };
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders tab links and loads billing data", async () => {
    renderWithQuery(<SettingsSurface activeTab={"billing" as SettingsTab} />);

    expect(screen.getByRole("link", { name: "Billing" }).getAttribute("href")).toBe(
      "/settings?tab=billing",
    );
    expect(screen.getByRole("link", { name: "Profile" }).getAttribute("href")).toBe(
      "/settings?tab=profile",
    );
    expect(screen.getByRole("link", { name: "Appearance" }).getAttribute("href")).toBe(
      "/settings?tab=appearance",
    );
    expect(screen.getByRole("link", { name: "Security" }).getAttribute("href")).toBe(
      "/settings?tab=security",
    );
    expect(screen.getByRole("link", { name: "Notifications" }).getAttribute("href")).toBe(
      "/settings?tab=notifications",
    );

    await waitFor(() => {
      expect(screen.getAllByText("Pro")).toHaveLength(2);
    });

    expect(mocks.getSubscriptionMock).toHaveBeenCalledWith("token-123");
    expect(mocks.getUsageMock).toHaveBeenCalledWith("token-123");
    expect(mocks.listPlansMock).toHaveBeenCalledWith("token-123");
  });

  it("updates profile and writes the returned user back to auth state", async () => {
    const user = userEvent.setup();

    renderWithQuery(<SettingsSurface activeTab="profile" />);

    const input = screen.getByLabelText("Name");
    await user.clear(input);
    await user.type(input, "Owner Updated");
    await user.click(screen.getByRole("button", { name: "Save profile" }));

    await waitFor(() => {
      expect(mocks.updateProfileMock).toHaveBeenCalledWith("token-123", "Owner Updated");
    });

    expect(mocks.updateUserMock).toHaveBeenCalledWith({
      id: "user-1",
      email: "owner@example.com",
      full_name: "Owner Updated",
    });
    expect(screen.getByText("Settings saved.")).toBeTruthy();
  });

  it("switches theme and locale from the appearance panel", async () => {
    const user = userEvent.setup();

    renderWithQuery(<SettingsSurface activeTab="appearance" />);

    await user.click(screen.getByRole("button", { name: /Dark/i }));
    await user.click(screen.getByRole("button", { name: /English/i }));

    expect(mocks.setThemeMock).toHaveBeenCalledWith("dark");
    expect(mocks.setLocaleMock).toHaveBeenCalledWith("en");
  });

  it("saves notification preferences and marks a notification as read", async () => {
    const user = userEvent.setup();

    renderWithQuery(<SettingsSurface activeTab="notifications" />);

    await waitFor(() => {
      expect(screen.getByText("Security alert")).toBeTruthy();
    });

    await user.click(screen.getByLabelText("Security alerts"));
    await user.click(screen.getByRole("button", { name: "Save notification settings" }));

    await waitFor(() => {
      expect(mocks.updateUserPreferencesMock).toHaveBeenCalledWith(
        "token-123",
        expect.objectContaining({
          notifications: expect.objectContaining({
            security_enabled: false,
          }),
        }),
      );
    });

    expect(screen.getByText("Settings saved.")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Mark as read" }));

    await waitFor(() => {
      expect(mocks.markNotificationReadMock).toHaveBeenCalledWith("token-123", "notif-1");
    });

    expect(screen.getByRole("button", { name: "Read" })).toBeTruthy();
  });

  it("changes password, clears auth, and redirects to login", async () => {
    const user = userEvent.setup();

    renderWithQuery(<SettingsSurface activeTab="security" />);

    await user.type(screen.getByLabelText("Current password"), "old-pass");
    await user.type(screen.getByLabelText("New password"), "new-pass");
    await user.click(screen.getByRole("button", { name: "Change password" }));

    await waitFor(() => {
      expect(mocks.changePasswordMock).toHaveBeenCalledWith("token-123", {
        old_password: "old-pass",
        new_password: "new-pass",
      });
    });

    expect(mocks.clearAuthMock).toHaveBeenCalledTimes(1);
    expect(mocks.replaceMock).toHaveBeenCalledWith("/login");
  });

  it("logs out and redirects to login", async () => {
    const user = userEvent.setup();

    renderWithQuery(<SettingsSurface activeTab="security" />);

    await user.click(screen.getByRole("button", { name: "Log out" }));

    await waitFor(() => {
      expect(mocks.logoutMock).toHaveBeenCalledTimes(1);
    });

    expect(mocks.replaceMock).toHaveBeenCalledWith("/login");
  });

  it("shows reset password entry only when enabled", () => {
    mocks.authState = {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner",
      },
      clearAuth: mocks.clearAuthMock,
      updateUser: mocks.updateUserMock,
      logout: mocks.logoutMock,
      passwordResetEnabled: false,
    };

    const { rerender } = renderWithQuery(<SettingsSurface activeTab="security" />);

    expect(screen.queryByRole("link", { name: "Reset password" })).toBeNull();

    mocks.authState = {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner",
      },
      clearAuth: mocks.clearAuthMock,
      updateUser: mocks.updateUserMock,
      logout: mocks.logoutMock,
      passwordResetEnabled: true,
    };

    rerender(rerenderWithQuery(<SettingsSurface activeTab="security" />));

    expect(screen.getByRole("link", { name: "Reset password" }).getAttribute("href")).toBe(
      "/reset-password",
    );
  });
});
