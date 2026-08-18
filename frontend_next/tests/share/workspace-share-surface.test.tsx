import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => mocks.uiPreferencesState,
}));

vi.mock("next/navigation", () => ({
  usePathname: () => "/dashboard/ws-1/share",
  useSearchParams: () => new URLSearchParams(),
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), prefetch: vi.fn() }),
}));

// AppTopBar heavy leaves are covered by dashboard-surface tests; stub them here.
vi.mock("../../components/account-menu", () => ({ AccountMenu: () => null }));
vi.mock("../../components/notifications/notification-bell", () => ({
  NotificationBell: () => null,
}));
vi.mock("../../components/plan-entry", () => ({ PlanEntry: () => null }));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual("../../lib/share/client");

  return {
    ...actual,
    buildShareUrl: mocks.buildShareUrlMock,
    getShareSettings: mocks.getShareSettingsMock,
    getShareQuota: mocks.getShareQuotaMock,
    listMembers: mocks.listMembersMock,
    updateShareSettings: mocks.updateShareSettingsMock,
    createShareLink: mocks.createShareLinkMock,
    revokeShareLink: mocks.revokeShareLinkMock,
    inviteMember: mocks.inviteMemberMock,
    getShareAnalytics: mocks.getShareAnalyticsMock,
    getShareAccessLogs: mocks.getShareAccessLogsMock,
    removeMember: mocks.removeMemberMock,
  };
});

vi.mock("../../lib/settings/client", async () => {
  const actual = await vi.importActual("../../lib/settings/client");

  return {
    ...actual,
    updateProfile: mocks.updateProfileMock,
  };
});

// API 访问区块已并入分享中心；密钥列表在这里不打真实请求。
vi.mock("../../lib/api-access/client", async () => {
  const actual = await vi.importActual("../../lib/api-access/client");

  return {
    ...actual,
    listApiKeys: vi.fn().mockResolvedValue({ api_keys: [] }),
  };
});

import { WorkspaceShareCenterSurface } from "../../components/share/workspace-share-surface";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceShareSurfaceMocks());



function recentViewsByDay() {
  const today = new Date();
  today.setUTCHours(0, 0, 0, 0);

  const earlierDay = new Date(today);
  earlierDay.setUTCDate(today.getUTCDate() - 2);

  const latestDay = new Date(today);
  latestDay.setUTCDate(today.getUTCDate() - 1);

  return {
    [earlierDay.toISOString().slice(0, 10)]: 8,
    [latestDay.toISOString().slice(0, 10)]: 4,
  };
}

function renderWithQuery(ui: ReactElement) {
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

  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

describe("WorkspaceShareCenterSurface", () => {
  beforeEach(() => {
    mocks.authState = {
      token: "token-123",
    };
    mocks.uiPreferencesState = {
      locale: "en",
    };
    mocks.buildShareUrlMock.mockReset();
    mocks.getShareSettingsMock.mockReset();
    mocks.getShareQuotaMock.mockReset();
    mocks.listMembersMock.mockReset();
    mocks.updateShareSettingsMock.mockReset();
    mocks.createShareLinkMock.mockReset();
    mocks.revokeShareLinkMock.mockReset();
    mocks.inviteMemberMock.mockReset();
    mocks.getShareAnalyticsMock.mockReset();
    mocks.getShareAccessLogsMock.mockReset();
    mocks.removeMemberMock.mockReset();

    mocks.buildShareUrlMock.mockImplementation((token: string) =>
      token ? `https://app.example.test/shared/kb/${token}` : "",
    );
    mocks.getShareSettingsMock.mockResolvedValue({
      share_token: "share-123",
      access_level: "link",
      expires_at: "2026-04-30T18:00:00Z",
      allow_download: true,
      anon_question_limit: 10,
      member_question_limit: null,
    });
    mocks.getShareQuotaMock.mockResolvedValue({
      used: 1,
      max: 3,
      plan_id: "free",
    });
    mocks.listMembersMock.mockResolvedValue({
      members: [
        {
          member_id: "member-1",
          user_id: "user-2",
          email: "member@example.com",
          role: "viewer",
          status: "pending",
          invited_at: "1713369600",
        },
      ],
    });
    mocks.updateShareSettingsMock.mockResolvedValue({
      share_token: "share-123",
      access_level: "public",
      expires_at: "2026-04-30T18:00:00Z",
      allow_download: true,
      anon_question_limit: 10,
      member_question_limit: null,
    });
    mocks.createShareLinkMock.mockResolvedValue({ share_token: "share-123" });
    mocks.getShareAnalyticsMock.mockResolvedValue({
      total_views: 12,
      total_unique_visitors: 3,
      views_by_day: recentViewsByDay(),
    });
    mocks.getShareAccessLogsMock.mockResolvedValue({
      logs: [
        {
          id: "log-1",
          visitor_id: "visitor-a",
          accessed_at: "1713369600",
          action: "view",
        },
      ],
    });
    mocks.updateProfileMock.mockReset();
  });

  it("loads share settings, members, and analytics through react-query", async () => {
    const user = userEvent.setup();

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    expect(
      await screen.findByText("https://app.example.test/shared/kb/share-123"),
    ).toBeTruthy();
    expect(screen.getByLabelText("Validity")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Members & permissions" }));
    expect(await screen.findByText("member@example.com")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Traffic" }));
    expect(await screen.findByText("Distribution overview")).toBeTruthy();

    await waitFor(() => {
      expect(mocks.getShareSettingsMock).toHaveBeenCalledWith("token-123", "ws-1");
      expect(mocks.listMembersMock).toHaveBeenCalledWith("token-123", "ws-1");
      expect(mocks.getShareAnalyticsMock).toHaveBeenCalledWith("token-123", "ws-1");
    });
  });

  it("requires owner-cost confirm before enabling share and shows quota", async () => {
    const user = userEvent.setup();

    mocks.getShareSettingsMock.mockResolvedValue({
      share_token: "",
      access_level: "private",
      expires_at: null,
      allow_download: false,
      anon_question_limit: 10,
      member_question_limit: null,
    });
    mocks.updateShareSettingsMock.mockResolvedValue({
      share_token: "share-456",
      access_level: "link",
      expires_at: null,
      allow_download: false,
      anon_question_limit: 10,
      member_question_limit: null,
    });

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    expect((await screen.findAllByText("Inactive")).length).toBeGreaterThan(0);
    expect(await screen.findByTestId("share-quota")).toHaveTextContent(
      "1 used / 3 max (free)",
    );

    await user.selectOptions(screen.getByLabelText("Validity"), "never");
    await user.click(screen.getByRole("switch"));

    // Toggle alone must not create a link — force confirm first.
    expect(mocks.createShareLinkMock).not.toHaveBeenCalled();
    expect(await screen.findByTestId("share-enable-confirm")).toBeTruthy();
    expect(
      screen.getByText(/Model usage and API costs from visitors on this share are billed to you/i),
    ).toBeTruthy();

    await user.click(screen.getByTestId("share-enable-confirm-action"));

    await waitFor(() => {
      expect(mocks.createShareLinkMock).toHaveBeenCalledWith("token-123", "ws-1", {
        role: "viewer",
        expires_at: null,
      });
    });

    await waitFor(() => {
      expect(mocks.updateShareSettingsMock).toHaveBeenCalledWith("token-123", "ws-1", {
        access_level: "link",
        allow_download: false,
      });
    });
  });

  it("surfaces a friendly message when share quota is exceeded", async () => {
    const user = userEvent.setup();
    const { ApiError } = await import("../../lib/http/request");

    mocks.getShareSettingsMock.mockResolvedValue({
      share_token: "",
      access_level: "private",
      expires_at: null,
      allow_download: false,
      anon_question_limit: 10,
      member_question_limit: null,
    });
    mocks.createShareLinkMock.mockRejectedValue(
      new ApiError(403, "share_workspace_quota_exceeded", "plan allows at most 3"),
    );

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await screen.findAllByText("Inactive");
    await user.click(screen.getByRole("switch"));
    await user.click(await screen.findByTestId("share-enable-confirm-action"));

    expect(
      await screen.findByText(
        "You have reached the shareable workspace limit. Disable sharing on another workspace or upgrade your plan.",
      ),
    ).toBeTruthy();
  });

  it("maps visitor mode to access_level when share is already live", async () => {
    const user = userEvent.setup();
    const futureExpiry = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString();

    mocks.getShareSettingsMock.mockResolvedValue({
      share_token: "share-123",
      access_level: "link",
      expires_at: futureExpiry,
      allow_download: true,
      anon_question_limit: 10,
      member_question_limit: null,
    });
    mocks.updateShareSettingsMock.mockResolvedValue({
      share_token: "share-123",
      access_level: "public",
      expires_at: futureExpiry,
      allow_download: true,
      anon_question_limit: 10,
      member_question_limit: null,
    });

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await screen.findByText("https://app.example.test/shared/kb/share-123");
    await user.selectOptions(screen.getByLabelText("Visitor mode"), "anonymous");

    await waitFor(() => {
      expect(mocks.updateShareSettingsMock).toHaveBeenCalledWith("token-123", "ws-1", {
        access_level: "public",
        allow_download: true,
      });
    });
  });

  it("validates invite email before submitting and then sends the invite", async () => {
    const user = userEvent.setup();

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await user.click(screen.getByRole("button", { name: "Members & permissions" }));
    await user.type(await screen.findByLabelText("Invite email"), "invalid-email");
    await user.click(screen.getByRole("button", { name: "Send invite" }));

    expect(await screen.findByText("Enter a valid email address.")).toBeTruthy();
    expect(mocks.inviteMemberMock).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText("Invite email"));
    await user.type(screen.getByLabelText("Invite email"), "new-member@example.com");
    await user.selectOptions(screen.getByLabelText("Invite role"), "editor");
    await user.click(screen.getByRole("button", { name: "Send invite" }));

    await waitFor(() => {
      expect(mocks.inviteMemberMock).toHaveBeenCalledWith(
        "token-123",
        "ws-1",
        "new-member@example.com",
        "editor",
      );
    });
  });

  it("uses an explicit remove flow instead of browser confirm", async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.spyOn(window, "confirm").mockImplementation(() => true);

    mocks.removeMemberMock.mockResolvedValue({});
    mocks.listMembersMock
      .mockResolvedValueOnce({
        members: [
          {
            member_id: "member-1",
            user_id: "user-2",
            email: "member@example.com",
            role: "viewer",
            status: "pending",
            invited_at: "1713369600",
          },
        ],
      })
      .mockResolvedValueOnce({
        members: [],
      });

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await user.click(screen.getByRole("button", { name: "Members & permissions" }));
    await screen.findByText("member@example.com");
    await user.click(screen.getByRole("button", { name: "Remove" }));

    expect(confirmSpy).not.toHaveBeenCalled();
    expect(mocks.removeMemberMock).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Confirm remove" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Confirm remove" }));

    await waitFor(() => {
      expect(mocks.removeMemberMock).toHaveBeenCalledWith("token-123", "ws-1", "member-1");
    });

    confirmSpy.mockRestore();
  });

  it("loads analytics in the overview section", async () => {
    const user = userEvent.setup();

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await user.click(screen.getByRole("button", { name: "Traffic" }));
    const overviewHeading = await screen.findByText("Distribution overview");
    const overviewSection = overviewHeading.closest("section");

    expect(overviewSection).toBeTruthy();
    expect(within(overviewSection as HTMLElement).getByText("Total views")).toBeTruthy();
    expect(within(overviewSection as HTMLElement).getByText("Active days in last 30 days")).toBeTruthy();

    await waitFor(() => {
      expect(within(overviewSection as HTMLElement).getAllByText("12").length).toBeGreaterThan(0);
      expect(within(overviewSection as HTMLElement).getByText("2")).toBeTruthy();
    });

    await waitFor(() => {
      expect(mocks.getShareAnalyticsMock).toHaveBeenCalledWith("token-123", "ws-1");
    });
  });

  it("hides the owner profile card when there is no signed-in user", async () => {
    const user = userEvent.setup();

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await screen.findByText("https://app.example.test/shared/kb/share-123");
    await user.click(screen.getByRole("button", { name: "Sharer profile" }));
    expect(screen.queryByTestId("owner-profile-card")).toBeNull();
  });

  it("toggles the public sharer profile via updateProfile with the full profile", async () => {
    const user = userEvent.setup();
    const updateUser = vi.fn();
    const authUser = {
      id: "user-1",
      email: "owner@example.com",
      full_name: "Owner Name",
      bio: "hello",
      contact_url: "https://example.com",
      public_profile_enabled: false,
    };
    mocks.authState = {
      token: "token-123",
      user: authUser,
      updateUser,
    };
    mocks.updateProfileMock.mockResolvedValue({
      success: true,
      data: {
        token: "",
        user: { ...authUser, public_profile_enabled: true },
        reset_ticket: null,
      },
      error: null,
    });

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await user.click(screen.getByRole("button", { name: "Sharer profile" }));
    const card = await screen.findByTestId("owner-profile-card");
    const toggle = within(card).getByTestId("owner-profile-switch");
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    expect(within(card).queryByTestId("owner-profile-public-link")).toBeNull();

    await user.click(toggle);

    await waitFor(() => {
      expect(mocks.updateProfileMock).toHaveBeenCalledWith("token-123", {
        full_name: "Owner Name",
        bio: "hello",
        contact_url: "https://example.com",
        public_profile_enabled: true,
      });
    });
    await waitFor(() => {
      expect(updateUser).toHaveBeenCalledWith(
        expect.objectContaining({ public_profile_enabled: true }),
      );
    });
  });

  it("surfaces an error when the owner profile toggle fails", async () => {
    const user = userEvent.setup();
    mocks.authState = {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner Name",
        public_profile_enabled: true,
      },
      updateUser: vi.fn(),
    };
    mocks.updateProfileMock.mockResolvedValue({
      success: false,
      data: null,
      error: "boom",
    });

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await user.click(screen.getByRole("button", { name: "Sharer profile" }));
    const card = await screen.findByTestId("owner-profile-card");
    expect(
      within(card).getByTestId("owner-profile-public-link").getAttribute("href"),
    ).toBe("/shared/u/user-1");

    await user.click(within(card).getByTestId("owner-profile-switch"));

    expect(await within(card).findByText("boom")).toBeTruthy();
  });
});
