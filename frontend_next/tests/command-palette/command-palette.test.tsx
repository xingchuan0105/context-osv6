import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const pushMock = vi.fn();
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

vi.mock("@/lib/search/client", () => ({
  searchProductIndex: (...args: unknown[]) => searchProductIndexMock(...args),
}));

import { CommandPaletteHost } from "@/components/command-palette/command-palette";

describe("CommandPaletteHost", () => {
  beforeEach(() => {
    pushMock.mockReset();
    listWorkspacesMock.mockReset();
    searchProductIndexMock.mockReset();
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
          workspace_id: "ws-2",
          title: "季度复盘",
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

    expect(await screen.findByTestId("command-palette-item-sess-sess-9")).toBeTruthy();
    expect(screen.getByTestId("command-palette-item-src-src-1")).toBeTruthy();

    fireEvent.click(screen.getByTestId("command-palette-item-sess-sess-9"));
    expect(pushMock).toHaveBeenCalledWith("/dashboard/ws-2?session=sess-9");
    expect(JSON.parse(window.localStorage.getItem("context-os.command-palette.recent-workspaces.v1") ?? "[]")).toEqual([
      "ws-2",
    ]);
  });
});
