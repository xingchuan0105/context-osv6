import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConversationScopeKind } from "@/lib/contracts/generated";

const pushMock = vi.fn();
const listChatSessionsMock = vi.fn();
const listWorkspacesMock = vi.fn();
const searchProductIndexMock = vi.fn();

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: pushMock }),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

vi.mock("@/lib/auth/context", () => ({
  useAuth: () => ({ token: "token-1", isAuthenticated: true, initialized: true }),
}));

vi.mock("@/lib/dashboard/client", () => ({
  listWorkspaces: (...args: unknown[]) => listWorkspacesMock(...args),
}));

vi.mock("@/lib/chat/client", () => ({
  listChatSessions: (...args: unknown[]) => listChatSessionsMock(...args),
  sortConversationsByRecent: (sessions: Array<{ id: string; updated_at: string }>) =>
    [...sessions].sort(
      (left, right) =>
        right.updated_at.localeCompare(left.updated_at) || left.id.localeCompare(right.id),
    ),
}));

vi.mock("@/lib/search/client", () => ({
  searchProductIndex: (...args: unknown[]) => searchProductIndexMock(...args),
}));

import { CommandPaletteHost } from "@/components/command-palette/command-palette";

describe("CommandPaletteHost", () => {
  beforeEach(() => {
    pushMock.mockReset();
    listChatSessionsMock.mockReset();
    listWorkspacesMock.mockReset();
    searchProductIndexMock.mockReset();
    listChatSessionsMock.mockResolvedValue({ sessions: [] });
    listWorkspacesMock.mockResolvedValue({ workspaces: [] });
    searchProductIndexMock.mockResolvedValue({
      workspaces: [],
      sessions: [],
      sources: [],
    });
    window.localStorage.clear();
  });

  it("opens on Ctrl+K and navigates to top-up", async () => {
    render(<CommandPaletteHost />);

    expect(screen.queryByTestId("command-palette")).toBeNull();

    fireEvent.keyDown(document, { key: "k", ctrlKey: true });
    expect(screen.getByTestId("command-palette")).toBeTruthy();
    expect(screen.getByTestId("command-palette-item-dashboard")).toBeTruthy();

    fireEvent.click(screen.getByTestId("command-palette-item-topup"));
    expect(pushMock).toHaveBeenCalledWith("/pricing#topup");
  });

  it("filters static commands by query", () => {
    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", metaKey: true });

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "充值" },
    });

    expect(screen.getByTestId("command-palette-item-topup")).toBeTruthy();
    expect(screen.queryByTestId("command-palette-item-dashboard")).toBeNull();
  });

  it("shows server-backed recent conversations and opens a personal chat", async () => {
    listChatSessionsMock.mockResolvedValue({
      sessions: [
        {
          id: "personal-recent",
          owner_user_id: "user-1",
          scope_kind: ConversationScopeKind.Personal,
          title: "最近的个人对话",
          agent_type: "chat",
          model_role: "quick_chat",
          pinned: false,
          created_at: "2026-09-01T00:00:00Z",
          updated_at: "2026-09-02T00:00:00Z",
        },
      ],
    });

    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });

    expect(
      await screen.findByTestId("command-palette-item-sess-personal-recent"),
    ).toBeTruthy();
    fireEvent.click(screen.getByTestId("command-palette-item-sess-personal-recent"));

    expect(pushMock).toHaveBeenCalledWith("/chat/personal-recent");
    expect(
      JSON.parse(
        window.localStorage.getItem(
          "context-os.command-palette.recent-workspaces.v1",
        ) ?? "[]",
      ),
    ).toEqual([]);
  });

  it("lists and opens workspaces from local list before global search returns", async () => {
    listWorkspacesMock.mockResolvedValue({
      workspaces: [
        {
          workspace_id: "ws-1",
          title: "研究笔记",
          name: "研究笔记",
          description: "",
          document_count: 1,
          status_summary: {},
          shared: false,
        },
      ],
    });
    // Keep global search pending so local list path is exercised.
    searchProductIndexMock.mockImplementation(() => new Promise(() => {}));

    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });

    await waitFor(() => {
      expect(listWorkspacesMock).toHaveBeenCalledWith("token-1");
    });

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "研究" },
    });

    expect(await screen.findByTestId("command-palette-item-ws-ws-1")).toBeTruthy();
    fireEvent.click(screen.getByTestId("command-palette-item-ws-ws-1"));
    expect(pushMock).toHaveBeenCalledWith("/dashboard/ws-1");
    expect(JSON.parse(window.localStorage.getItem("context-os.command-palette.recent-workspaces.v1") ?? "[]")).toEqual([
      "ws-1",
    ]);
  });

  it("opens a session deep-link from global search", async () => {
    searchProductIndexMock.mockResolvedValue({
      workspaces: [],
      sessions: [
        {
          id: "sess-9",
          owner_user_id: "user-1",
          workspace_id: "ws-2",
          scope_kind: ConversationScopeKind.Workspace,
          workspace_name: "项目库",
          title: "季度复盘",
          agent_type: "chat",
          model_role: "agent",
          pinned: false,
          created_at: "2026-07-31T00:00:00Z",
          updated_at: "2026-08-01T00:00:00Z",
        },
      ],
      sources: [
        {
          id: "src-1",
          workspace_id: "ws-2",
          file_name: "notes.pdf",
          title: "notes",
          workspace_name: "项目库",
        },
      ],
    });

    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "复盘" },
    });

    await waitFor(() => {
      expect(searchProductIndexMock).toHaveBeenCalledWith("token-1", "复盘");
    });

    expect(
      await screen.findByTestId("command-palette-item-sess-sess-9"),
    ).toHaveTextContent("会话 · 季度复盘 · 工作区 · 项目库");
    expect(screen.getByTestId("command-palette-item-src-src-1")).toBeTruthy();

    fireEvent.click(screen.getByTestId("command-palette-item-sess-sess-9"));
    expect(pushMock).toHaveBeenCalledWith("/dashboard/ws-2?session=sess-9");
    expect(JSON.parse(window.localStorage.getItem("context-os.command-palette.recent-workspaces.v1") ?? "[]")).toEqual([
      "ws-2",
    ]);
  });

  it("opens a personal session deep-link from global search", async () => {
    searchProductIndexMock.mockResolvedValue({
      workspaces: [],
      sessions: [
        {
          id: "personal-9",
          owner_user_id: "user-1",
          scope_kind: ConversationScopeKind.Personal,
          title: "个人复盘",
          agent_type: "chat",
          model_role: "quick_chat",
          pinned: false,
          created_at: "2026-07-31T00:00:00Z",
          updated_at: "2026-08-01T00:00:00Z",
        },
      ],
      sources: [],
    });

    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });
    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "个人复盘" },
    });

    expect(
      await screen.findByTestId("command-palette-item-sess-personal-9"),
    ).toHaveTextContent("会话 · 个人复盘 · 本对话");
    fireEvent.click(screen.getByTestId("command-palette-item-sess-personal-9"));

    expect(pushMock).toHaveBeenCalledWith("/chat/personal-9");
    expect(
      window.localStorage.getItem("context-os.command-palette.recent-workspaces.v1"),
    ).toBeNull();
  });

  it("does not fall back to local workspaces after search returns no workspace hits", async () => {
    window.localStorage.setItem(
      "context-os.command-palette.recent-workspaces.v1",
      JSON.stringify(["ws-local"]),
    );
    listWorkspacesMock.mockResolvedValue({
      workspaces: [
        {
          workspace_id: "ws-local",
          title: "本地工作区",
          name: "本地工作区",
          description: "",
          document_count: 0,
          status_summary: {},
          shared: false,
        },
      ],
    });
    searchProductIndexMock.mockResolvedValue({
      workspaces: [],
      sessions: [
        {
          id: "personal-only-hit",
          owner_user_id: "user-1",
          scope_kind: ConversationScopeKind.Personal,
          title: "唯一命中的个人对话",
          agent_type: "chat",
          model_role: "quick_chat",
          pinned: false,
          created_at: "2026-07-31T00:00:00Z",
          updated_at: "2026-08-01T00:00:00Z",
        },
      ],
      sources: [],
    });

    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });

    expect(
      await screen.findByTestId("command-palette-item-ws-ws-local"),
    ).toBeTruthy();
    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "唯一命中" },
    });

    expect(
      await screen.findByTestId("command-palette-item-sess-personal-only-hit"),
    ).toBeTruthy();
    expect(
      screen.queryByTestId("command-palette-item-ws-ws-local"),
    ).toBeNull();
  });

  it("opens a source deep-link from global search", async () => {
    searchProductIndexMock.mockResolvedValue({
      workspaces: [],
      sessions: [],
      sources: [
        {
          id: "src-7",
          workspace_id: "ws-3",
          file_name: "合同.pdf",
          title: "合同",
          workspace_name: "法务库",
        },
      ],
    });

    render(<CommandPaletteHost />);
    fireEvent.keyDown(document, { key: "k", ctrlKey: true });

    fireEvent.change(screen.getByTestId("command-palette-input"), {
      target: { value: "合同" },
    });

    expect(await screen.findByTestId("command-palette-item-src-src-7")).toBeTruthy();
    fireEvent.click(screen.getByTestId("command-palette-item-src-src-7"));
    expect(pushMock).toHaveBeenCalledWith("/dashboard/ws-3?source=src-7");
    expect(JSON.parse(window.localStorage.getItem("context-os.command-palette.recent-workspaces.v1") ?? "[]")).toEqual([
      "ws-3",
    ]);
  });
});
