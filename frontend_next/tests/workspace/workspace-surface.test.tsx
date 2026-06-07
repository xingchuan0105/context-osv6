import { fireEvent, render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  pushMock: vi.fn(),
  replaceMock: vi.fn(),
  createWorkspaceMock: vi.fn(),
  getDefaultWorkspaceTitleMock: vi.fn(),
  markDefaultWorkspaceTitleUsedMock: vi.fn(),
  setLocaleMock: vi.fn(),
  setThemeMock: vi.fn(),
  logoutMock: vi.fn(),
  authState: {
    initialized: true,
    token: "token-123",
    isAuthenticated: true,
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  },
  getWorkspaceMock: vi.fn(),
  listWorkspaceSessionsMock: vi.fn(),
  listWorkspaceSessionMessagesMock: vi.fn(),
  createWorkspaceSessionMock: vi.fn(),
  updateWorkspaceMock: vi.fn(),
  updateWorkspaceSessionMock: vi.fn(),
  deleteWorkspaceSessionMock: vi.fn(),
  lookupWorkspaceCitationMock: vi.fn(),
  chatPaneMock: vi.fn(),
  rightRailMock: vi.fn(),
  getUsageWindowMock: vi.fn(),
}));

let mobileViewport = false;
const matchMediaListeners = new Set<(event: MediaQueryListEvent) => void>();

function setMobileViewport(next: boolean) {
  mobileViewport = next;
  for (const listener of matchMediaListeners) {
    listener({ matches: next } as MediaQueryListEvent);
  }
}

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: mocks.pushMock,
    replace: mocks.replaceMock,
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
    setLocale: mocks.setLocaleMock,
    setTheme: mocks.setThemeMock,
  }),
}));

vi.mock("../../lib/dashboard/client", () => ({
  createWorkspace: mocks.createWorkspaceMock,
}));

vi.mock("../../lib/dashboard/default-title", () => ({
  getDefaultWorkspaceTitle: mocks.getDefaultWorkspaceTitleMock,
  markDefaultWorkspaceTitleUsed: mocks.markDefaultWorkspaceTitleUsedMock,
}));

vi.mock("../../lib/workspace/client", () => ({
  getWorkspace: mocks.getWorkspaceMock,
  listWorkspaceSessions: mocks.listWorkspaceSessionsMock,
  listWorkspaceSessionMessages: mocks.listWorkspaceSessionMessagesMock,
  createWorkspaceSession: mocks.createWorkspaceSessionMock,
  updateWorkspace: mocks.updateWorkspaceMock,
  updateWorkspaceSession: mocks.updateWorkspaceSessionMock,
  deleteWorkspaceSession: mocks.deleteWorkspaceSessionMock,
  lookupWorkspaceCitation: mocks.lookupWorkspaceCitationMock,
}));

vi.mock("../../lib/billing/api", () => ({
  billingApi: {
    getUsageWindow: mocks.getUsageWindowMock,
  },
}));

vi.mock("../../lib/billing/featureFlag", () => ({
  isPricingRevampEnabledSSR: () => true,
  isPricingRevampEnabled: vi.fn().mockResolvedValue(true),
}));

vi.mock("../../components/workspace/workspace-chat-pane", () => ({
  WorkspaceChatPane: (props: {
    workspaceId: string;
    sessionId: string | null;
    selectedSourceIds: string[];
    onSessionActivity?: () => void;
    onSessionChange?: (sessionId: string | null) => void;
    onFocusSource?: (sourceId: string | null) => void;
    onOpenWebSources?: (request: {
      sources: Array<{
        title: string;
        url: string;
        snippet: string;
      }>;
    }) => void;
    onSelectCitation?: (request: {
      session_id: string;
      message_id: number;
      citation: {
        citation_id: number;
        doc_id: string;
        doc_name: string;
        score: number;
      };
      anchorRect?: {
        top: number;
        left: number;
        right: number;
        bottom: number;
        width: number;
        height: number;
      } | null;
    }) => void;
  }) => {
    mocks.chatPaneMock(props);

    return (
      <section aria-label="Workspace chat">
        <div>Chat session: {props.sessionId ?? "none"}</div>
        <div>Selected sources: {props.selectedSourceIds.join(",") || "none"}</div>
        <button type="button" onClick={() => props.onSessionActivity?.()}>
          Refresh sessions
        </button>
        <button type="button" onClick={() => props.onSessionChange?.("sess-chat")}>
          Adopt chat session
        </button>
        <button type="button" onClick={() => props.onFocusSource?.("src-1")}>
          Focus source src-1
        </button>
        <button
          type="button"
          onClick={() =>
            props.onOpenWebSources?.({
              sources: [
                {
                  title: "Search Result",
                  url: "https://example.test/search",
                  snippet: "Search snippet",
                },
              ],
            })
          }
        >
          Open web sources
        </button>
        <button
          type="button"
          onClick={() =>
            props.onSelectCitation?.({
              session_id: "sess-1",
              message_id: 7,
              citation: { citation_id: 1, doc_id: "src-2", doc_name: "Source Two", score: 0.9 },
              anchorRect: {
                top: 120,
                left: 260,
                right: 284,
                bottom: 144,
                width: 24,
                height: 24,
              },
            })
          }
        >
          Select citation src-2
        </button>
      </section>
    );
  },
}));

vi.mock("../../components/workspace/workspace-right-rail", () => ({
  WorkspaceRightRail: (props: {
    workspaceId: string;
    selectedSourceIds: string[];
    onSelectedSourceIdsChange: (ids: string[]) => void;
    focusedSourceId?: string | null;
    activeWebSources?: {
      sources: Array<{
        title: string;
        url: string;
        snippet: string;
      }>;
    } | null;
    onCloseWebSources?: () => void;
  }) => {
    mocks.rightRailMock(props);

    return (
      <div>
        <div>Focused source: {props.focusedSourceId ?? "none"}</div>
        <div>
          Web sources:{" "}
          {props.activeWebSources?.sources.map((source) => source.title).join(",") || "none"}
        </div>
        <button type="button" onClick={() => props.onSelectedSourceIdsChange(["src-1"])}>
          Select src-1
        </button>
        {props.activeWebSources ? (
          <button type="button" onClick={() => props.onCloseWebSources?.()}>
            Close web sources
          </button>
        ) : null}
      </div>
    );
  },
}));

import { WorkspaceSurface } from "../../components/workspace/workspace-surface";
import { workspaceUiStore } from "../../lib/workspace/ui-store";

beforeEach(() => {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn().mockImplementation(() => ({
      matches: mobileViewport,
      media: "(max-width: 767px)",
      onchange: null,
      addEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.add(listener);
      },
      removeEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.delete(listener);
      },
      addListener: (listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.add(listener);
      },
      removeListener: (listener: (event: MediaQueryListEvent) => void) => {
        matchMediaListeners.delete(listener);
      },
      dispatchEvent: vi.fn(),
    })),
  });

  setMobileViewport(false);
  window.localStorage.clear();
  workspaceUiStore.setState((state) => ({ ...state, workspaces: {} }));
  mocks.pushMock.mockReset();
  mocks.replaceMock.mockReset();
  mocks.createWorkspaceMock.mockReset();
  mocks.getDefaultWorkspaceTitleMock.mockReset();
  mocks.markDefaultWorkspaceTitleUsedMock.mockReset();
  mocks.setLocaleMock.mockReset();
  mocks.setThemeMock.mockReset();
  mocks.logoutMock.mockReset();
  mocks.getWorkspaceMock.mockReset();
  mocks.listWorkspaceSessionsMock.mockReset();
  mocks.listWorkspaceSessionMessagesMock.mockReset();
  mocks.createWorkspaceSessionMock.mockReset();
  mocks.updateWorkspaceMock.mockReset();
  mocks.updateWorkspaceSessionMock.mockReset();
  mocks.deleteWorkspaceSessionMock.mockReset();
  mocks.lookupWorkspaceCitationMock.mockReset();
  mocks.chatPaneMock.mockReset();
  mocks.rightRailMock.mockReset();
  mocks.getUsageWindowMock.mockReset();

  mocks.getUsageWindowMock.mockResolvedValue({
    plan_id: "free",
    rolling_5h: { used: 1000, limit: 100000, percentage: 1, reset_at: "2099-01-01T00:00:00Z" },
    rolling_7d: { used: 5000, limit: 400000, percentage: 1, reset_at: "2099-01-01T00:00:00Z" },
    soft_limit_hit: { rolling_5h: false, rolling_7d: false },
    hard_limit_hit: { rolling_5h: false, rolling_7d: false },
  });

  mocks.getWorkspaceMock.mockResolvedValue({
    workspace: {
      workspace_id: "ws-1",
      org_id: "org-1",
      owner_id: "user-1",
      name: "Workspace 1",
      title: "Workspace 1",
      description: "A workspace",
      created_at: "2026-04-17T00:00:00Z",
      updated_at: "2026-04-18T00:00:00Z",
      document_count: 2,
      status_summary: { ready: 1 },
      shared: false,
    },
  });
  mocks.listWorkspaceSessionsMock.mockResolvedValue({
    sessions: [
      {
        id: "sess-1",
        workspace_id: "ws-1",
        title: "Pinned thread",
        agent_type: "rag",
        summary: "Summary",
        pinned: true,
        created_at: "2026-04-17T00:00:00Z",
        updated_at: "2026-04-18T00:00:00Z",
      },
      {
        id: "sess-2",
        workspace_id: "ws-1",
        title: "General thread",
        agent_type: "rag",
        summary: null,
        pinned: false,
        created_at: "2026-04-16T00:00:00Z",
        updated_at: "2026-04-17T00:00:00Z",
      },
    ],
  });
  mocks.createWorkspaceSessionMock.mockResolvedValue({
    id: "sess-3",
    workspace_id: "ws-1",
    title: "New thread",
    agent_type: "rag",
    summary: null,
    pinned: false,
    created_at: "2026-04-19T00:00:00Z",
    updated_at: "2026-04-19T00:00:00Z",
  });
  mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
    messages: [],
  });
  mocks.updateWorkspaceSessionMock.mockImplementation(
    async (_token, sessionId, requestBody: { title?: string | null; pinned?: boolean | null }) => {
      if (requestBody.pinned !== undefined) {
        return {
          id: sessionId,
          workspace_id: "ws-1",
          title: "General thread",
          agent_type: "rag",
          summary: null,
          pinned: Boolean(requestBody.pinned),
          created_at: "2026-04-16T00:00:00Z",
          updated_at: "2026-04-19T00:00:00Z",
        };
      }

      return {
        id: sessionId,
        workspace_id: "ws-1",
        title: requestBody.title ?? null,
        agent_type: "rag",
        summary: null,
        pinned: false,
        created_at: "2026-04-16T00:00:00Z",
        updated_at: "2026-04-19T00:00:00Z",
      };
    },
  );
  mocks.updateWorkspaceMock.mockResolvedValue({
    workspace: {
      workspace_id: "ws-1",
      org_id: "org-1",
      owner_id: "user-1",
      name: "Workspace 1",
      title: "Renamed Workspace",
      description: "A workspace",
      created_at: "2026-04-17T00:00:00Z",
      updated_at: "2026-04-19T00:00:00Z",
      document_count: 2,
      status_summary: { ready: 1 },
      shared: false,
    },
  });
  mocks.deleteWorkspaceSessionMock.mockResolvedValue(undefined);
  mocks.lookupWorkspaceCitationMock.mockResolvedValue({
    doc_name: "Source Two",
    content: "Chunk detail for src-2",
    doc_id: "src-2",
    chunk_id: "chunk-1",
    page: 3,
    chunk_type: "text",
    asset_id: null,
    caption: null,
    image_url: null,
  });
  mocks.createWorkspaceMock.mockResolvedValue({
    workspace: {
      workspace_id: "ws-2",
      org_id: "org-1",
      owner_id: "user-1",
      name: "工作区1",
      title: "工作区1",
      description: "",
      created_at: "2026-04-19T00:00:00Z",
      updated_at: "2026-04-19T00:00:00Z",
      document_count: 0,
      status_summary: {},
      shared: false,
    },
  });
  mocks.getDefaultWorkspaceTitleMock.mockReturnValue("工作区1");
  mocks.authState = {
    initialized: true,
    token: "token-123",
    isAuthenticated: true,
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: mocks.logoutMock,
  };
});

afterEach(() => {
  matchMediaListeners.clear();
  vi.clearAllMocks();
});

describe("WorkspaceSurface", () => {
  it("renders the desktop shell, restores store-backed UI state, and wires the core workspace actions", async () => {
    const user = userEvent.setup();

    workspaceUiStore.getState().setSelectedSourceIds("ws-1", ["src-2"]);
    workspaceUiStore.getState().setFocusedSourceId("ws-1", "src-2");

    render(<WorkspaceSurface workspaceId="ws-1" />);

    expect(await screen.findByLabelText("工作区标题")).toBeTruthy();
    expect(screen.getByTestId("desktop-history-rail")).toBeTruthy();
    expect(screen.getByTestId("desktop-right-rail")).toBeTruthy();
    expect(screen.getByText("Selected sources: src-2")).toBeTruthy();
    expect(screen.getByText("Focused source: src-2")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "工作区标题" }));
    await user.clear(screen.getByLabelText("工作区标题"));
    await user.type(screen.getByLabelText("工作区标题"), "Renamed Workspace{enter}");

    await waitFor(() => {
      expect(mocks.updateWorkspaceMock).toHaveBeenCalledWith("token-123", "ws-1", {
        name: "Renamed Workspace",
        description: "A workspace",
      });
    });

    await user.click(screen.getByRole("button", { name: "新建会话" }));
    expect(screen.getByText("Chat session: none")).toBeTruthy();
    expect(mocks.createWorkspaceSessionMock).not.toHaveBeenCalled();

    const history = screen.getByRole("region", { name: "工作区历史" });
    await user.click(within(history).getByText("General thread"));
    expect(screen.getByText("Chat session: sess-2")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Focus source src-1" }));
    expect(screen.getByText("Focused source: src-1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Select src-1" }));
    expect(screen.getByText("Selected sources: src-1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Adopt chat session" }));
    await waitFor(() => {
      expect(mocks.listWorkspaceSessionsMock).toHaveBeenCalledTimes(2);
    });

    await user.click(screen.getByRole("button", { name: "创建工作区" }));

    await waitFor(() => {
      expect(mocks.markDefaultWorkspaceTitleUsedMock).toHaveBeenCalled();
      expect(mocks.createWorkspaceMock).toHaveBeenCalledWith("token-123", {
        name: "工作区1",
        description: "",
      });
    });
  });

  it("opens a citation chunk modal without refocusing the right rail", async () => {
    const user = userEvent.setup();

    workspaceUiStore.getState().setFocusedSourceId("ws-1", "src-1");

    render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");
    expect(screen.getByText("Focused source: src-1")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Select citation src-2" }));

    await waitFor(() => {
      expect(mocks.lookupWorkspaceCitationMock).toHaveBeenCalledWith("token-123", {
        session_id: "sess-1",
        message_id: 7,
        citation_id: 1,
      });
    });

    expect(screen.getByText("Focused source: src-1")).toBeTruthy();
    const dialog = screen.getByRole("dialog", { name: "引用片段" });
    expect(dialog).toBeTruthy();
    expect(screen.getByText("Chunk detail for src-2")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "关闭" })).toBeNull();

    await user.click(dialog.parentElement as HTMLElement);

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "引用片段" })).toBeNull();
    });
    expect(screen.getByText("Focused source: src-1")).toBeTruthy();
  });

  it("opens web sources in the right rail and clears the citation modal", async () => {
    const user = userEvent.setup();

    workspaceUiStore.getState().setRightRailOpen("ws-1", false);

    render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");
    await user.click(screen.getByRole("button", { name: "Select citation src-2" }));

    expect(await screen.findByRole("dialog", { name: "引用片段" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Open web sources" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "引用片段" })).toBeNull();
      expect(workspaceUiStore.getState().workspaces["ws-1"]?.rightRailOpen).toBe(true);
      expect(mocks.rightRailMock).toHaveBeenLastCalledWith(
        expect.objectContaining({
          activeWebSources: {
            sources: [
              {
                title: "Search Result",
                url: "https://example.test/search",
                snippet: "Search snippet",
              },
            ],
          },
        }),
      );
    });
    expect(screen.getByText("Web sources: Search Result")).toBeTruthy();
  });

  it("opens mobile history and right drawers from the stored toggle state", async () => {
    setMobileViewport(true);
    workspaceUiStore.getState().setHistoryRailOpen("ws-1", true);
    workspaceUiStore.getState().setRightRailOpen("ws-1", false);

    const firstRender = render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");
    expect(screen.getByTestId("mobile-history-drawer")).toBeTruthy();
    expect(screen.queryByTestId("mobile-right-drawer")).toBeNull();

    firstRender.unmount();

    workspaceUiStore.getState().setHistoryRailOpen("ws-1", false);
    workspaceUiStore.getState().setRightRailOpen("ws-1", true);

    render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");
    expect(screen.queryByTestId("mobile-history-drawer")).toBeNull();
    expect(screen.getByTestId("mobile-right-drawer")).toBeTruthy();
  });

  it("resizes desktop rails through the visible separators", async () => {
    render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");

    const [historyResizer] = screen.getAllByRole("separator");

    fireEvent.mouseDown(historyResizer, { clientX: 200 });
    fireEvent.mouseMove(window, { clientX: 180 });
    fireEvent.mouseUp(window);

    expect(workspaceUiStore.getState().workspaces["ws-1"]?.historyRailWidth).toBe(300);
  });

  it("supports pointer-based rail resizing for webview-style input", async () => {
    render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");

    const [, rightResizer] = screen.getAllByRole("separator");

    fireEvent.pointerDown(rightResizer, { clientX: 1200 });
    fireEvent.pointerMove(window, { clientX: 1120 });
    fireEvent.pointerUp(window);

    expect(workspaceUiStore.getState().workspaces["ws-1"]?.rightRailWidth).toBe(392);
  });

  it("supports touch-based rail resizing for embedded webviews", async () => {
    render(<WorkspaceSurface workspaceId="ws-1" />);

    await screen.findByLabelText("工作区标题");

    const [historyResizer] = screen.getAllByRole("separator");

    fireEvent.touchStart(historyResizer, {
      touches: [{ clientX: 200 }],
    });
    fireEvent.touchMove(window, {
      touches: [{ clientX: 260 }],
    });
    fireEvent.touchEnd(window);

    expect(workspaceUiStore.getState().workspaces["ws-1"]?.historyRailWidth).toBe(320);
  });

  it("renders warning toast when soft limit hit on 5h", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 85000, limit: 100000, percentage: 85, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });

    render(<WorkspaceSurface workspaceId="ws-1" />);
    await waitFor(() => expect(screen.getByText(/5h 用量已用 85%/)).toBeTruthy());
  });

  it("renders warning toast at 95% tier on 7d window", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 1000, limit: 100000, percentage: 1, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 390000, limit: 400000, percentage: 97, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: false, rolling_7d: true },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });

    render(<WorkspaceSurface workspaceId="ws-1" />);
    await waitFor(() => expect(screen.getByText(/7d 用量已用 97%/)).toBeTruthy());
  });

  it("redirects to paywall when hard limit hit", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 100000, limit: 100000, percentage: 100, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: true, rolling_7d: false },
    });

    render(<WorkspaceSurface workspaceId="ws-1" />);
    await waitFor(() => {
      expect(mocks.pushMock).toHaveBeenCalledWith("/upgrade/paywall?reason=5h");
    });
  });

  it("prefers higher-pressure window when both hard limits hit", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 100000, limit: 100000, percentage: 100, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 450000, limit: 400000, percentage: 100, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: true },
      hard_limit_hit: { rolling_5h: true, rolling_7d: true },
    });

    render(<WorkspaceSurface workspaceId="ws-1" />);
    await waitFor(() => {
      expect(mocks.pushMock).toHaveBeenCalledWith("/upgrade/paywall?reason=7d");
    });
  });

  it("prefers higher-pressure window when both soft limits hit", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 85000, limit: 100000, percentage: 85, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 390000, limit: 400000, percentage: 97, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: true },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });

    render(<WorkspaceSurface workspaceId="ws-1" />);
    await waitFor(() => expect(screen.getByText(/7d 用量已用 97%/)).toBeTruthy());
  });
});
