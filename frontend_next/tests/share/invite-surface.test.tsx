import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  pushMock: vi.fn(),
  authState: {
    initialized: true,
    token: null as string | null,
  },
  getWorkspaceMock: vi.fn(),
  acceptInviteMock: vi.fn(),
  declineInviteMock: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: mocks.pushMock,
    replace: vi.fn(),
    prefetch: vi.fn(),
    refresh: vi.fn(),
    back: vi.fn(),
    forward: vi.fn(),
  }),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/workspace/client", () => ({
  getWorkspace: mocks.getWorkspaceMock,
}));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual("../../lib/share/client");

  return {
    ...actual,
    acceptInvite: mocks.acceptInviteMock,
    declineInvite: mocks.declineInviteMock,
  };
});

import { InviteSurface } from "../../components/share/invite-surface";

describe("InviteSurface", () => {
  beforeEach(() => {
    mocks.pushMock.mockReset();
    mocks.getWorkspaceMock.mockReset();
    mocks.acceptInviteMock.mockReset();
    mocks.declineInviteMock.mockReset();
    mocks.getWorkspaceMock.mockResolvedValue({
      workspace: {
        workspace_id: "ws-1",
        org_id: "org-1",
        owner_id: "user-1",
        name: "Workspace One",
        title: "Workspace One",
        description: "",
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-18T00:00:00Z",
        document_count: 0,
        status_summary: {},
        shared: false,
      },
    });
    mocks.acceptInviteMock.mockResolvedValue(undefined);
    mocks.declineInviteMock.mockResolvedValue(undefined);
    mocks.authState = {
      initialized: true,
      token: null,
    };
  });

  it("keeps invite public and offers login/register continuation links", async () => {
    render(<InviteSurface memberId="member-1" workspaceId="ws-1" />);

    expect((await screen.findByRole("link", { name: "登录后继续" })).getAttribute("href")).toBe(
      "/login?next=%2Finvite%2Fws-1%2Fmember-1",
    );
    expect(screen.getByRole("link", { name: "注册后继续" }).getAttribute("href")).toBe(
      "/register?next=%2Finvite%2Fws-1%2Fmember-1",
    );
  });

  it("accepts the invite for authenticated users and links to the workspace", async () => {
    const user = userEvent.setup();
    mocks.authState = {
      initialized: true,
      token: "token-123",
    };

    render(<InviteSurface memberId="member-1" workspaceId="ws-1" />);

    await user.click(await screen.findByRole("button", { name: "接受邀请" }));

    await waitFor(() => {
      expect(mocks.acceptInviteMock).toHaveBeenCalledWith("token-123", "ws-1", "member-1");
    });

    expect(screen.getByRole("link", { name: "打开 Workspace" }).getAttribute("href")).toBe("/dashboard/ws-1");
  });
});
