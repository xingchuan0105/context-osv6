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

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane capabilities", () => {
  it("defaults to pure chat (empty capabilities) even when sources are selected", async () => {
    const user = userEvent.setup();
    const requests: Array<{
      agent_type?: string;
      workspace_id?: string;
      doc_scope?: string[];
      capabilities?: string[];
      client_context?: { local_time?: string; timezone?: string };
    }> = [];

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
      expect(requests[0]).toMatchObject({
        agent_type: "chat",
        capabilities: [],
      });
    });
    expect(requests[0]?.client_context?.timezone).toBeTruthy();
    expect(requests[0]?.client_context?.local_time).toMatch(/^\d{4}-\d{2}-\d{2}T/);

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
        agent_type: "chat",
        workspace_id: "ws-rag",
        doc_scope: ["doc-1"],
        capabilities: [],
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

  it("renders RAG and Search toggles without write mode", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(
      <WorkspaceChatPane
        workspaceId="ws-caps"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    expect(screen.getByTestId("workspace-chat-cap-rag")).toBeTruthy();
    expect(screen.getByTestId("workspace-chat-cap-search")).toBeTruthy();
    expect(screen.queryByTestId("workspace-chat-mode-write")).toBeNull();
    expect(screen.queryByTestId("workspace-chat-mode-menu")).toBeNull();
    expect(screen.queryByTestId("workspace-chat-write-usage-hint")).toBeNull();

    expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe("false");
    expect(screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed")).toBe(
      "false",
    );
  });

  it("toggles capabilities multiselect and sends derived agent_type", async () => {
    const user = userEvent.setup();
    const requests: Array<{
      agent_type?: string;
      capabilities?: string[];
      client_context?: { local_time?: string; timezone?: string };
    }> = [];

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

    render(
      <WorkspaceChatPane
        workspaceId="ws-toggle-caps"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const rag = screen.getByTestId("workspace-chat-cap-rag");
    const search = screen.getByTestId("workspace-chat-cap-search");

    await user.click(search);
    expect(search.getAttribute("aria-pressed")).toBe("true");
    expect(rag.getAttribute("aria-pressed")).toBe("false");

    await user.click(rag);
    expect(rag.getAttribute("aria-pressed")).toBe("true");
    expect(search.getAttribute("aria-pressed")).toBe("true");

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(composer, "Dual caps");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(requests[0]).toMatchObject({
        agent_type: "rag+search",
        capabilities: ["rag", "search"],
      });
    });
    expect(requests[0]?.client_context?.timezone).toBeTruthy();
  });

  it("can select search alone and shows search chip after reply", async () => {
    const user = userEvent.setup();
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      await onEvent({
        event: "done",
        request_id: "req-1",
        session_id: "sess-1",
        message_id: 1,
        payload: {
          answer: "search answer",
          answer_blocks: [],
          session_id: "sess-1",
          agent_type: request.agent_type,
          sources: [],
          citations: [],
          trace: { mode: request.agent_type ?? "search" },
          degrade_trace: [],
        },
      });
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-search-only"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    await user.click(screen.getByTestId("workspace-chat-cap-search"));
    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(composer, "Search please");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(mocks.streamWorkspaceChatMock).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          agent_type: "search",
          capabilities: ["search"],
        }),
        expect.any(Function),
        expect.anything(),
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("capability-chip-search")).toBeTruthy();
    });
    expect(screen.queryByTestId("capability-chip-rag")).toBeNull();
  });
});
