import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceChatPaneMocks());

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.useAuthMock(),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/workspace/client", () => ({
  listWorkspaceSessionMessages: mocks.listWorkspaceSessionMessagesMock,
}));

vi.mock("../../lib/runtime/transport", () => ({
  streamChat: mocks.streamWorkspaceChatMock,
}));

import { mockReducedMotionPreference, resetWorkspaceChatPaneMocks } from "./helpers/workspace-chat-pane.setup";

import { WorkspaceChatPane } from "../../components/workspace/workspace-chat-pane";
import { workspaceUiStore } from "../../lib/workspace/ui-store";

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane modes", () => {
  it("uses chat by default without sources and rag by default when sources are available", async () => {
    const user = userEvent.setup();
    const requests: Array<{ agent_type?: string; workspace_id?: string; doc_scope?: string[] }> = [];

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      requests.push(request);

      await onEvent({
        event: "done",
        request_id: `req-${requests.length}`,
        session_id: `sess-${requests.length}`,
        message_id: requests.length,
        payload: {
          answer: "ok",
          answer_blocks: [],
          session_id: `sess-${requests.length}`,
          agent_type: request.agent_type,
          sources: [],
          citations: [],
          trace: { mode: request.agent_type ?? "general" },
          degrade_trace: [],
        },
      });
    });

    const firstRender = render(
      <WorkspaceChatPane
        workspaceId="ws-empty"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const firstComposer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(firstComposer, "Hello");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(requests[0]?.agent_type).toBe("chat");
    });

    firstRender.unmount();

    render(
      <WorkspaceChatPane
        workspaceId="ws-rag"
        sessionId={null}
        selectedSourceIds={["doc-1"]}
      />,
    );

    const secondComposer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(secondComposer, "What is in the doc?");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(requests[1]).toMatchObject({
        agent_type: "rag",
        workspace_id: "ws-rag",
        doc_scope: ["doc-1"],
      });
    });
  });

  it("treats Shift+Enter as a newline instead of a submit", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(
      <WorkspaceChatPane
        workspaceId="ws-1"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const user = userEvent.setup();
    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });

    await user.type(composer, "Line 1");
    await user.keyboard("{Shift>}{Enter}{/Shift}");
    await user.type(composer, "Line 2");

    expect((composer as HTMLTextAreaElement).value).toBe("Line 1\nLine 2");
    expect(mocks.streamWorkspaceChatMock).not.toHaveBeenCalled();
  });

  it("opens the mode menu on hover (U14) and keeps it open across the bridge delay", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    const matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query.includes("hover: hover"),
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
    vi.stubGlobal("matchMedia", matchMedia);

    render(
      <WorkspaceChatPane
        workspaceId="ws-hover-mode"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const anchor = screen.getByTestId("workspace-chat-mode-anchor");
    expect(screen.queryByTestId("workspace-chat-mode-menu")).toBeNull();

    fireEvent.mouseEnter(anchor);

    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-mode-menu")).toBeTruthy();
    });
    expect(screen.getByTestId("workspace-chat-mode-rag")).toBeTruthy();
    expect(screen.getByTestId("workspace-chat-mode-search")).toBeTruthy();
    expect(screen.getByTestId("workspace-chat-mode-chat")).toBeTruthy();
    expect(screen.getByTestId("workspace-chat-mode-write")).toBeTruthy();
    expect(screen.getByText("快速即时对话")).toBeTruthy();

    fireEvent.mouseLeave(anchor);
    // Still open during close delay (U14 bridge).
    expect(screen.getByTestId("workspace-chat-mode-menu")).toBeTruthy();

    await waitFor(
      () => {
        expect(screen.queryByTestId("workspace-chat-mode-menu")).toBeNull();
      },
      { timeout: 500 },
    );

    vi.unstubAllGlobals();
  });

  it("toggles the mode menu on click and applies selection immediately", async () => {
    const user = userEvent.setup();
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(
      <WorkspaceChatPane
        workspaceId="ws-click-mode"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const modeButton = screen.getByTestId("workspace-chat-mode-button");
    await user.click(modeButton);
    expect(screen.getByTestId("workspace-chat-mode-menu")).toBeTruthy();

    await user.click(screen.getByTestId("workspace-chat-mode-search"));
    expect(screen.queryByTestId("workspace-chat-mode-menu")).toBeNull();
    expect(workspaceUiStore.getState().workspaces["ws-click-mode"]?.chatMode).toBe("search");
    expect(within(modeButton).getByText("网络搜索")).toBeTruthy();
  });
});
