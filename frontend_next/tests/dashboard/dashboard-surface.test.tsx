import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetDefaultWorkspaceTitleCounters } from "../../lib/dashboard/default-title";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createDashboardSurfaceMocks());

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

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/dashboard/client", () => ({
  createWorkspace: mocks.createWorkspaceMock,
  deleteWorkspace: mocks.deleteWorkspaceMock,
  listWorkspaces: mocks.listWorkspacesMock,
  updateWorkspace: mocks.updateWorkspaceMock,
}));

vi.mock("../../lib/dashboard/preferences", () => ({
  getFavoriteWorkspaceIds: mocks.getFavoriteWorkspaceIdsMock,
  updateFavoriteWorkspaceIds: mocks.updateFavoriteWorkspaceIdsMock,
}));

vi.mock("../../lib/settings/client", () => ({
  getUsageLimit: mocks.getUsageLimitMock,
}));

import { DashboardSurface } from "../../components/dashboard/dashboard-surface";

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

const workspaces = [
  {
    workspace_id: "ws-1",
    owner_user_id: "owner-1",
    owner_id: "user-1",
    name: "Alpha",
    title: "Alpha",
    description: "First workspace",
    created_at: "2026-04-15T08:00:00Z",
    updated_at: "2026-04-16T08:00:00Z",
    document_count: 3,
    status_summary: { ready: 1 },
    shared: false,
  },
  {
    workspace_id: "ws-2",
    owner_user_id: "owner-1",
    owner_id: "user-2",
    name: "Beta",
    title: "Beta",
    description: "Second workspace",
    created_at: "2026-04-15T08:00:00Z",
    updated_at: "2026-04-17T08:00:00Z",
    document_count: 8,
    status_summary: { processing: 2 },
    shared: true,
  },
  {
    workspace_id: "ws-3",
    owner_user_id: "owner-1",
    owner_id: "user-1",
    name: "Gamma",
    title: "",
    description: "",
    created_at: "2026-04-14T08:00:00Z",
    updated_at: "2026-04-15T08:00:00Z",
    document_count: 0,
    status_summary: {},
    shared: false,
  },
] as const;

beforeEach(() => {
  window.localStorage.clear();
  resetDefaultWorkspaceTitleCounters();
  mocks.pushMock.mockReset();
  mocks.listWorkspacesMock.mockReset();
  mocks.getFavoriteWorkspaceIdsMock.mockReset();
  mocks.createWorkspaceMock.mockReset();
  mocks.updateWorkspaceMock.mockReset();
  mocks.deleteWorkspaceMock.mockReset();
  mocks.updateFavoriteWorkspaceIdsMock.mockReset();
  mocks.getUsageLimitMock.mockReset();
  mocks.authState = {
    initialized: true,
    isAuthenticated: true,
    token: "token-123",
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  };

  mocks.listWorkspacesMock.mockResolvedValue({
    workspaces: [...workspaces],
  });
  mocks.getFavoriteWorkspaceIdsMock.mockResolvedValue(["ws-2"]);
  mocks.createWorkspaceMock.mockResolvedValue({
    workspace: {
      ...workspaces[0],
      workspace_id: "ws-4",
      name: "工作区1",
      title: "工作区1",
      created_at: "2026-04-18T08:00:00Z",
      updated_at: "2026-04-18T08:00:00Z",
    },
  });
  mocks.updateWorkspaceMock.mockResolvedValue({
    workspace: {
      ...workspaces[2],
      workspace_id: "ws-3",
      title: "Renamed Gamma",
      name: "Renamed Gamma",
      updated_at: "2026-04-18T08:00:00Z",
    },
  });
  mocks.deleteWorkspaceMock.mockResolvedValue(undefined);
  mocks.updateFavoriteWorkspaceIdsMock.mockResolvedValue(["ws-2", "ws-1"]);
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
});

afterEach(() => {
  vi.clearAllMocks();
});

async function openWorkspaceMenu(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.click(screen.getByRole("button", { name: `${title} 操作` }));
  return screen.getByRole("menu", { name: `${title} 操作` });
}

describe("DashboardSurface", () => {
  it("renders card view by default and makes workspace content clickable outside the actions menu", async () => {
    const user = userEvent.setup();

    renderWithQuery(<DashboardSurface />);

    await waitFor(() => {
      expect(mocks.listWorkspacesMock).toHaveBeenCalledTimes(1);
    });

    const workspaceGrid = await screen.findByRole("grid", { name: "工作区卡片" });
    expect(within(workspaceGrid).getAllByRole("gridcell").length).toBe(3);

    expect(screen.getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "我的工作区" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "我的收藏" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "卡片" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("button", { name: "列表" }).getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("button", { name: "搜索工作区" })).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "新建工作区" })).toHaveLength(2);

    expect(screen.getByRole("link", { name: "Beta" }).getAttribute("href")).toBe("/dashboard/ws-2");
    expect(screen.getByRole("link", { name: "Alpha" }).getAttribute("href")).toBe("/dashboard/ws-1");
    expect(screen.getByRole("link", { name: "Gamma" }).getAttribute("href")).toBe("/dashboard/ws-3");
    expect(screen.getByText("8 个来源").closest("a")?.getAttribute("href")).toBe("/dashboard/ws-2");
    expect(screen.getByRole("button", { name: "Beta 操作" }).closest("a")).toBeNull();

    expect(screen.getByRole("button", { name: "Beta 操作" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Alpha 操作" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Gamma 操作" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "列表" }));

    const workspaceList = await screen.findByRole("list", { name: "工作区列表" });
    expect(within(workspaceList).getAllByRole("listitem").length).toBe(3);
    expect(within(workspaceList).getByText("8 个来源").closest("a")?.getAttribute("href")).toBe("/dashboard/ws-2");
    expect(screen.getByRole("button", { name: "Beta 操作" }).closest("a")).toBeNull();
  });

  it("switches tabs, sort mode, view mode, and favorites", async () => {
    const user = userEvent.setup();

    renderWithQuery(<DashboardSurface />);

    await screen.findByRole("grid", { name: "工作区卡片" });

    await user.click(screen.getByRole("button", { name: "我的工作区" }));
    const mineGrid = screen.getByRole("grid", { name: "工作区卡片" });
    expect(within(mineGrid).getAllByRole("gridcell")).toHaveLength(2);
    expect(within(mineGrid).getByRole("link", { name: "Alpha" })).toBeTruthy();
    expect(within(mineGrid).getByRole("link", { name: "Gamma" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "创建时间" }));
    await user.click(screen.getByRole("menuitem", { name: "标题" }));
    const sortedMineGrid = screen.getByRole("grid", { name: "工作区卡片" });
    expect(within(sortedMineGrid).getAllByRole("link").map((link) => link.getAttribute("aria-label"))).toEqual([
      "Alpha",
      "Gamma",
    ]);

    await user.click(screen.getByRole("button", { name: "列表" }));
    expect(screen.getByRole("list", { name: "工作区列表" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "卡片" }).getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByRole("button", { name: "列表" }).getAttribute("aria-pressed")).toBe("true");

    const gammaMenu = await openWorkspaceMenu(user, "Gamma");
    await user.click(within(gammaMenu).getByRole("menuitem", { name: "收藏" }));
    await waitFor(() => {
      expect(mocks.updateFavoriteWorkspaceIdsMock).toHaveBeenCalledWith("token-123", ["ws-2", "ws-3"]);
    });

    await user.click(screen.getByRole("button", { name: "卡片" }));
    await user.click(screen.getByRole("button", { name: "我的收藏" }));
    expect(within(screen.getByRole("grid", { name: "工作区卡片" })).getAllByRole("gridcell")).toHaveLength(2);
  });

  it("uses the empty-state CTA to create a workspace immediately", async () => {
    const user = userEvent.setup();
    mocks.listWorkspacesMock.mockResolvedValue({ workspaces: [] });
    mocks.getFavoriteWorkspaceIdsMock.mockResolvedValue([]);

    renderWithQuery(<DashboardSurface />);

    await screen.findByText("创建第一个工作区");

    await user.click(screen.getByRole("button", { name: "创建第一个工作区" }));
    await waitFor(() => {
      expect(mocks.createWorkspaceMock).toHaveBeenCalledWith("token-123", {
        name: "工作区1",
        description: "",
      });
      expect(mocks.pushMock).toHaveBeenCalledWith("/dashboard/ws-4");
    });
  });

  it("creates, searches, renames, and deletes workspaces against the dashboard contract", async () => {
    const user = userEvent.setup();
    const promptMock = vi.spyOn(window, "prompt").mockReturnValue("Renamed Gamma");
    const confirmMock = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderWithQuery(<DashboardSurface />);

    await screen.findByRole("grid", { name: "工作区卡片" });

    await user.click(screen.getAllByRole("button", { name: "新建工作区" })[0]);

    await waitFor(() => {
      expect(mocks.createWorkspaceMock).toHaveBeenCalledWith("token-123", {
        name: "工作区1",
        description: "",
      });
    });
    expect(mocks.pushMock).toHaveBeenCalledWith("/dashboard/ws-4");

    await user.click(screen.getByRole("button", { name: "搜索工作区" }));
    const searchDialog = screen.getByRole("dialog", { name: "快速打开工作区" });
    await user.type(within(searchDialog).getByLabelText("搜索工作区"), "gamma");
    expect(within(searchDialog).getByRole("link", { name: "Gamma" }).getAttribute("href")).toBe(
      "/dashboard/ws-3",
    );

    await user.click(screen.getByRole("button", { name: "关闭搜索" }));
    const renameMenu = await openWorkspaceMenu(user, "Gamma");
    await user.click(within(renameMenu).getByRole("menuitem", { name: "重命名" }));
    await waitFor(() => {
      expect(mocks.updateWorkspaceMock).toHaveBeenCalledWith("token-123", "ws-3", {
        name: "Renamed Gamma",
        description: "",
      });
    });

    const betaMenu = await openWorkspaceMenu(user, "Beta");
    await user.click(within(betaMenu).getByRole("menuitem", { name: "删除" }));
    await waitFor(() => {
      expect(confirmMock).toHaveBeenCalledWith("删除 Beta?");
      expect(mocks.deleteWorkspaceMock).toHaveBeenCalledWith("token-123", "ws-2");
      expect(mocks.updateFavoriteWorkspaceIdsMock).toHaveBeenCalledWith("token-123", []);
    });

    promptMock.mockRestore();
    confirmMock.mockRestore();
  });
});
