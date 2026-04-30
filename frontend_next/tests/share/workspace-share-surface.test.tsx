import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
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
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => mocks.uiPreferencesState,
}));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual("../../lib/share/client");

  return {
    ...actual,
    buildShareUrl: mocks.buildShareUrlMock,
    getShareSettings: mocks.getShareSettingsMock,
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

import { WorkspaceShareCenterSurface } from "../../components/share/workspace-share-surface";

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
    });
    mocks.createShareLinkMock.mockResolvedValue({ share_token: "share-123" });
    mocks.getShareAnalyticsMock.mockResolvedValue({
      total_views: 12,
      total_unique_visitors: 3,
      views_by_day: {
        "2026-04-17": 8,
        "2026-04-18": 4,
      },
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
  });

  it("loads share settings, members, and analytics through react-query", async () => {
    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    expect(
      await screen.findByText("https://app.example.test/shared/kb/share-123"),
    ).toBeTruthy();
    expect(screen.getByText("member@example.com")).toBeTruthy();
    expect(screen.getByLabelText("Validity")).toBeTruthy();
    expect(screen.getByText("Distribution overview")).toBeTruthy();

    await waitFor(() => {
      expect(mocks.getShareSettingsMock).toHaveBeenCalledWith("token-123", "ws-1");
      expect(mocks.listMembersMock).toHaveBeenCalledWith("token-123", "ws-1");
      expect(mocks.getShareAnalyticsMock).toHaveBeenCalledWith("token-123", "ws-1");
    });
  });

  it("generates the first share link with the selected validity window", async () => {
    const user = userEvent.setup();

    mocks.getShareSettingsMock.mockResolvedValue({
      share_token: "",
      access_level: "private",
      expires_at: null,
      allow_download: false,
    });
    mocks.updateShareSettingsMock.mockResolvedValue({
      share_token: "share-456",
      access_level: "link",
      expires_at: null,
      allow_download: false,
    });

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    expect((await screen.findAllByText("Inactive")).length).toBeGreaterThan(0);
    await user.selectOptions(screen.getByLabelText("Validity"), "never");
    await user.click(screen.getByRole("switch"));

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

  it("validates invite email before submitting and then sends the invite", async () => {
    const user = userEvent.setup();

    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

    await screen.findByText("Members & permissions");
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
    renderWithQuery(<WorkspaceShareCenterSurface workspaceId="ws-1" />);

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
});
