import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  pathname: "/chat",
  search: "",
  replace: vi.fn(),
  getChatSession: vi.fn(),
  listChatSessions: vi.fn(),
  listWorkspaces: vi.fn(),
  canvasProps: [] as Array<Record<string, unknown>>,
  canvasMounts: 0,
  canvasUnmounts: 0,
}));

vi.mock("next/navigation", () => {
  const router = {
    replace: (...args: unknown[]) => mocks.replace(...args),
  };
  return {
    usePathname: () => mocks.pathname,
    useRouter: () => router,
    useSearchParams: () => new URLSearchParams(mocks.search),
  };
});

vi.mock("@/lib/auth/context", () => ({
  useAuth: () => ({
    initialized: true,
    isAuthenticated: true,
    token: "token-1",
  }),
}));

vi.mock("@/lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const, theme: "system" as const }),
}));

vi.mock("@/lib/chat/client", () => ({
  getChatSession: (...args: unknown[]) => mocks.getChatSession(...args),
  listChatSessions: (...args: unknown[]) => mocks.listChatSessions(...args),
  sortConversationsByRecent: (
    sessions: Array<{ id: string; updated_at: string }>,
  ) =>
    [...sessions].sort(
      (left, right) =>
        right.updated_at.localeCompare(left.updated_at) || left.id.localeCompare(right.id),
    ),
}));

vi.mock("@/lib/dashboard/client", () => ({
  listWorkspaces: (...args: unknown[]) => mocks.listWorkspaces(...args),
}));

vi.mock("@/components/app-top-bar", () => ({
  AppTopBar: () => <div data-testid="app-top-bar" />,
}));

vi.mock("@/components/chat/chat-canvas", async () => {
  const React = await import("react");
  return {
    ChatCanvas: (props: Record<string, unknown>) => {
      mocks.canvasProps.push(props);
      React.useEffect(() => {
        mocks.canvasMounts += 1;
        return () => {
          mocks.canvasUnmounts += 1;
        };
      }, []);
      return <div data-testid="chat-canvas-mock" />;
    },
  };
});

vi.mock("@/components/workspace/workspace-web-sources-modal", () => ({
  WorkspaceWebSourcesModal: () => null,
}));

import { ChatShell } from "@/components/chat/chat-shell";

const baseSession = {
  owner_user_id: "user-1",
  scope_kind: "personal",
  agent_type: "chat",
  model_role: "quick_chat",
  created_at: "2026-09-01T00:00:00Z",
};

beforeEach(() => {
  mocks.pathname = "/chat";
  mocks.search = "";
  mocks.replace.mockReset();
  mocks.getChatSession.mockReset();
  mocks.listChatSessions.mockReset();
  mocks.listWorkspaces.mockReset();
  mocks.canvasProps.length = 0;
  mocks.canvasMounts = 0;
  mocks.canvasUnmounts = 0;
  mocks.listChatSessions.mockResolvedValue({ sessions: [] });
  mocks.listWorkspaces.mockResolvedValue({ workspaces: [] });
});

describe("ChatShell", () => {
  it("shows mixed global recents and mounts the personal shared canvas", async () => {
    mocks.listChatSessions.mockResolvedValue({
      sessions: [
        {
          ...baseSession,
          id: "workspace-old",
          workspace_id: "ws-1",
          scope_kind: "workspace",
          workspace_name: "产品研究",
          title: "工作区旧对话",
          pinned: true,
          updated_at: "2026-09-01T00:00:00Z",
        },
        {
          ...baseSession,
          id: "personal-new",
          title: "个人新对话",
          pinned: false,
          updated_at: "2026-09-02T00:00:00Z",
        },
      ],
    });
    mocks.listWorkspaces.mockResolvedValue({
      workspaces: [
        {
          workspace_id: "ws-1",
          title: "产品研究",
          name: "产品研究",
        },
      ],
    });

    render(<ChatShell />);

    const recent = await screen.findByRole("navigation", { name: "最近" });
    const links = within(recent).getAllByRole("link");
    expect(links.map((link) => link.getAttribute("href"))).toEqual([
      "/chat/personal-new",
      "/dashboard/ws-1?session=workspace-old",
    ]);
    expect(within(recent).getByText("本对话")).toBeTruthy();
    expect(within(recent).getByText("工作区 · 产品研究")).toBeTruthy();

    const latestCanvasProps = mocks.canvasProps.at(-1);
    expect(latestCanvasProps).toMatchObject({
      availableCapabilities: ["search"],
      selectedSourceIds: [],
      sessionId: null,
      workspaceId: null,
    });

    act(() => {
      (latestCanvasProps?.onSessionChange as (sessionId: string) => void)(
        "personal-created",
      );
    });
    expect(mocks.replace).toHaveBeenCalledWith("/chat/personal-created");
  });

  it("gates a deep link until exact owner-scoped lookup confirms it is personal", async () => {
    let resolveSession!: (session: Record<string, unknown>) => void;
    mocks.pathname = "/chat/personal-history";
    mocks.getChatSession.mockReturnValue(
      new Promise((resolve) => {
        resolveSession = resolve;
      }),
    );

    render(<ChatShell />);

    expect(screen.queryByTestId("chat-canvas-mock")).toBeNull();
    expect(mocks.getChatSession).toHaveBeenCalledWith("token-1", "personal-history");

    await act(async () => {
      resolveSession({
        ...baseSession,
        id: "personal-history",
        updated_at: "2026-09-02T00:00:00Z",
      });
    });

    expect(await screen.findByTestId("chat-canvas-mock")).toBeTruthy();
    expect(mocks.canvasProps.at(-1)).toMatchObject({
      sessionId: "personal-history",
      workspaceId: null,
    });
  });

  it("redirects a Workspace-bound session away from the personal route without mounting Canvas", async () => {
    mocks.pathname = "/chat/workspace-session";
    mocks.getChatSession.mockResolvedValue({
      ...baseSession,
      id: "workspace-session",
      workspace_id: "ws-9",
      scope_kind: "workspace",
      title: "Workspace chat",
      updated_at: "2026-09-02T00:00:00Z",
    });

    render(<ChatShell />);

    await waitFor(() => {
      expect(mocks.replace).toHaveBeenCalledWith(
        "/dashboard/ws-9?session=workspace-session",
      );
    });
    expect(screen.queryByTestId("chat-canvas-mock")).toBeNull();
  });

  it("keeps a failed exact-session lookup terminal and non-sendable", async () => {
    mocks.pathname = "/chat/missing-session";
    mocks.getChatSession.mockRejectedValue(new Error("not found"));

    render(<ChatShell />);

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.queryByTestId("chat-canvas-mock")).toBeNull();
  });

  it("retries the same exact-session lookup from its error terminal", async () => {
    mocks.pathname = "/chat/personal-history";
    mocks.getChatSession
      .mockRejectedValueOnce(new Error("temporary failure"))
      .mockResolvedValueOnce({
        ...baseSession,
        id: "personal-history",
        updated_at: "2026-09-02T00:00:00Z",
      });

    render(<ChatShell />);

    const error = await screen.findByRole("alert");
    expect(screen.queryByTestId("chat-canvas-mock")).toBeNull();
    fireEvent.click(within(error).getByRole("button", { name: "重试" }));

    expect(await screen.findByTestId("chat-canvas-mock")).toBeTruthy();
    expect(mocks.getChatSession).toHaveBeenCalledTimes(2);
    expect(mocks.getChatSession).toHaveBeenLastCalledWith(
      "token-1",
      "personal-history",
    );
  });

  it("keeps Canvas mounted when the first stream start assigns the personal session URL", async () => {
    const view = render(<ChatShell />);
    const firstCanvasProps = mocks.canvasProps.at(-1);

    act(() => {
      (firstCanvasProps?.onSessionChange as (sessionId: string) => void)("created-session");
    });
    mocks.pathname = "/chat/created-session";
    view.rerender(<ChatShell />);

    await waitFor(() => {
      expect(mocks.canvasProps.at(-1)?.sessionId).toBe("created-session");
    });
    expect(mocks.getChatSession).not.toHaveBeenCalled();
    expect(mocks.canvasMounts).toBe(1);
    expect(mocks.canvasUnmounts).toBe(0);
  });

  it("disables rail navigation while streaming and resets an already-new chat explicitly", async () => {
    render(<ChatShell />);
    const firstProps = mocks.canvasProps.at(-1);
    const initialEpoch = firstProps?.resetEpoch as number;

    act(() => {
      (firstProps?.onStreamingChange as (streaming: boolean) => void)(true);
    });

    const newChat = screen.getByTestId("chat-new-conversation");
    newChat.addEventListener("click", (event) => event.preventDefault());
    expect(newChat.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(newChat);
    expect(mocks.canvasProps.at(-1)?.resetEpoch).toBe(initialEpoch);

    act(() => {
      (mocks.canvasProps.at(-1)?.onStreamingChange as (streaming: boolean) => void)(false);
    });
    fireEvent.click(newChat);
    await waitFor(() => {
      expect(mocks.canvasProps.at(-1)?.resetEpoch).toBe(initialEpoch + 1);
    });
  });
});
