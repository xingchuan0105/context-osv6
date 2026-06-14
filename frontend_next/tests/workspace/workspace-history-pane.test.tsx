import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";


vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({
    initialized: true,
    token: "token-123",
    isAuthenticated: true,
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    updateUser: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  }),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "en" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/workspace/client", () => ({
  listWorkspaceSessionMessages: mocks.listWorkspaceSessionMessagesMock,
}));

import { WorkspaceHistoryPane } from "../../components/workspace/workspace-history-pane";
import { queryLibraryStore } from "../../lib/workspace/query-library/store";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceHistoryPaneMocks());



const sessions = [
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
];

beforeEach(() => {
  document.body.innerHTML = "";
  window.localStorage.clear();
  queryLibraryStore.setState({ workspaces: {} });
  mocks.listWorkspaceSessionMessagesMock.mockReset();
  mocks.listWorkspaceSessionMessagesMock.mockImplementation(async (_token: string, sessionId: string) => ({
    messages:
      sessionId === "sess-2"
        ? [
            {
              id: 21,
              session_id: "sess-2",
              role: "user",
              content: "Need the Phoenix migration timeline and deployment notes",
              answer_blocks: [],
              citations: [],
              created_at: "2026-04-18T00:00:00Z",
            },
          ]
        : [
            {
              id: 11,
              session_id: "sess-1",
              role: "user",
              content: "Discuss quarterly budget planning",
              answer_blocks: [],
              citations: [],
              created_at: "2026-04-18T00:00:00Z",
            },
          ],
  }));
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("WorkspaceHistoryPane", () => {
  it("mounts the query library panel in the history rail", () => {
    render(
      <WorkspaceHistoryPane
        activeSessionId="sess-1"
        onDeleteSession={vi.fn()}
        onInsert={vi.fn()}
        onNewThread={vi.fn()}
        onRenameSession={vi.fn()}
        onSelectSession={vi.fn()}
        onTogglePinSession={vi.fn()}
        sessions={sessions}
        workspaceId="ws-test"
      />,
    );

    expect(screen.getByTestId("query-library-panel")).toBeTruthy();
  });

  it("keeps session threads and query library visible together in the history rail", () => {
    const manySessions = Array.from({ length: 8 }, (_, index) => ({
      id: `sess-${index + 1}`,
      workspace_id: "ws-1",
      title: `Thread ${index + 1}`,
      agent_type: "rag",
      summary: null,
      pinned: index === 0,
      created_at: `2026-04-${String(index + 1).padStart(2, "0")}T00:00:00Z`,
      updated_at: `2026-04-${String(index + 2).padStart(2, "0")}T00:00:00Z`,
    }));

    for (let index = 0; index < 6; index += 1) {
      queryLibraryStore.getState().capture("ws-layout", `Saved prompt ${index + 1}`);
    }

    render(
      <WorkspaceHistoryPane
        activeSessionId="sess-1"
        onDeleteSession={vi.fn()}
        onInsert={vi.fn(() => true)}
        onNewThread={vi.fn()}
        onRenameSession={vi.fn()}
        onSelectSession={vi.fn()}
        onTogglePinSession={vi.fn()}
        sessions={manySessions}
        workspaceId="ws-layout"
      />,
    );

    expect(screen.getByText("Thread 1")).toBeTruthy();
    expect(screen.getByText("Thread 8")).toBeTruthy();
    expect(screen.getByTestId("query-library-panel")).toBeTruthy();
    expect(screen.getByText("Saved prompt 1")).toBeTruthy();
    expect(screen.getByText("Saved prompt 6")).toBeTruthy();
  });

  it("opens and closes the session kebab menu on outside click and Escape, then auto-closes after actions", async () => {
    const user = userEvent.setup();
    const onTogglePinSession = vi.fn();
    const onRenameSession = vi.fn();
    const onDeleteSession = vi.fn();

    render(
      <WorkspaceHistoryPane
        activeSessionId="sess-1"
        onDeleteSession={onDeleteSession}
        onInsert={vi.fn()}
        onNewThread={vi.fn()}
        onRenameSession={onRenameSession}
        onSelectSession={vi.fn()}
        onTogglePinSession={onTogglePinSession}
        sessions={sessions}
        workspaceId="ws-test"
      />,
    );

    expect(screen.getByRole("button", { name: "New session" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Search sessions" })).toBeTruthy();
    expect(screen.queryByPlaceholderText("Search sessions")).toBeNull();
    expect(screen.queryByText("Threads")).toBeNull();

    await user.click(screen.getByRole("button", { name: "General thread actions" }));
    const menu = screen.getByRole("menu", { name: "General thread actions" });
    expect(menu).toBeTruthy();
    expect(menu.closest("article")).toBeNull();

    await user.click(document.body);
    expect(screen.queryByRole("menu", { name: "General thread actions" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "General thread actions" }));
    expect(screen.getByRole("menu", { name: "General thread actions" })).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu", { name: "General thread actions" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "General thread actions" }));
    await user.click(within(screen.getByRole("menu", { name: "General thread actions" })).getByRole("menuitem", { name: "Pin" }));
    expect(onTogglePinSession).toHaveBeenCalledWith(expect.objectContaining({ id: "sess-2" }));
    expect(screen.queryByRole("menu", { name: "General thread actions" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "General thread actions" }));
    await user.click(
      within(screen.getByRole("menu", { name: "General thread actions" })).getByRole("menuitem", {
        name: "Rename",
      }),
    );
    expect(onRenameSession).toHaveBeenCalledWith(expect.objectContaining({ id: "sess-2" }));
    expect(screen.queryByRole("menu", { name: "General thread actions" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "General thread actions" }));
    await user.click(
      within(screen.getByRole("menu", { name: "General thread actions" })).getByRole("menuitem", {
        name: "Delete",
      }),
    );
    expect(onDeleteSession).toHaveBeenCalledWith(expect.objectContaining({ id: "sess-2" }));
    expect(screen.queryByRole("menu", { name: "General thread actions" })).toBeNull();
  });

  it("opens the search dialog and finds sessions by chat body text", async () => {
    const user = userEvent.setup();
    const onSelectSession = vi.fn();

    render(
      <WorkspaceHistoryPane
        activeSessionId="sess-1"
        onDeleteSession={vi.fn()}
        onInsert={vi.fn()}
        onNewThread={vi.fn()}
        onRenameSession={vi.fn()}
        onSelectSession={onSelectSession}
        onTogglePinSession={vi.fn()}
        sessions={sessions}
        workspaceId="ws-test"
      />,
    );

    await user.click(screen.getByRole("button", { name: "Search sessions" }));

    expect(screen.getByRole("dialog", { name: "Search sessions" })).toBeTruthy();
    expect(mocks.listWorkspaceSessionMessagesMock).toHaveBeenCalledWith("token-123", "sess-1");
    expect(mocks.listWorkspaceSessionMessagesMock).toHaveBeenCalledWith("token-123", "sess-2");

    await user.type(screen.getByRole("textbox", { name: "Search sessions" }), "phoenix");

    const results = await screen.findByRole("list", { name: "Session search results" });
    expect(within(results).getByRole("button", { name: "General thread" })).toBeTruthy();
    expect(screen.getByText(/Phoenix migration timeline/i)).toBeTruthy();

    await user.click(within(results).getByRole("button", { name: "General thread" }));

    expect(onSelectSession).toHaveBeenCalledWith("sess-2");
    expect(screen.queryByRole("dialog", { name: "Search sessions" })).toBeNull();
  });
});
