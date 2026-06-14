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
import { queryLibraryStore } from "../../lib/workspace/query-library/store";
import { workspaceUiStore } from "../../lib/workspace/ui-store";

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane composer", () => {
  it("renders a draggable composer handle and resizes the textarea", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(<WorkspaceChatPane workspaceId="ws-resize" sessionId={null} selectedSourceIds={[]} />);

    const composer = await screen.findByRole("textbox", { name: "工作区对话输入框" });
    const resizeHandle = screen.getByRole("button", { name: "调整输入框高度" });
    const initialHeight = Number.parseFloat((composer as HTMLTextAreaElement).style.height || "0");

    fireEvent.mouseDown(resizeHandle, { button: 0, clientY: 240 });
    fireEvent.mouseMove(window, { clientY: 180 });
    fireEvent.mouseUp(window);

    await waitFor(() => {
      const resizedHeight = Number.parseFloat((composer as HTMLTextAreaElement).style.height || "0");
      expect(resizedHeight).toBeGreaterThan(initialHeight);
    });
  });

  it("captures sent prompts when the user sends a message", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(
      <WorkspaceChatPane
        selectedSourceIds={[]}
        sessionId={null}
        workspaceId="ws-capture"
      />,
    );

    const composer = await screen.findByRole("textbox", { name: "工作区对话输入框" });
    fireEvent.change(composer, { target: { value: "Summarize quarterly report" } });
    fireEvent.keyDown(composer, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(mocks.streamWorkspaceChatMock).toHaveBeenCalled();
    });
    expect(queryLibraryStore.getState().workspaces["ws-capture"]?.[0]?.text).toBe(
      "Summarize quarterly report",
    );
  });

  it("does not capture prompts when auth token is missing", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.useAuthMock.mockReturnValue({
      initialized: true,
      isAuthenticated: false,
      token: null,
      user: null,
      passwordResetEnabled: true,
      completeAuth: vi.fn(),
      updateUser: vi.fn(),
      clearAuth: vi.fn(),
      logout: vi.fn(),
    });

    render(
      <WorkspaceChatPane
        selectedSourceIds={[]}
        sessionId={null}
        workspaceId="ws-no-token"
      />,
    );

    const composer = await screen.findByRole("textbox", { name: "工作区对话输入框" });
    fireEvent.change(composer, { target: { value: "Summarize quarterly report" } });
    fireEvent.keyDown(composer, { key: "Enter", code: "Enter" });

    expect(mocks.streamWorkspaceChatMock).not.toHaveBeenCalled();
    expect(queryLibraryStore.getState().workspaces["ws-no-token"]).toBeUndefined();
  });

  it("returns false from composer insert while streaming", async () => {
    vi.useFakeTimers();
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        event: "answer_start",
        request_id: "req-stream",
        session_id: "sess-stream",
        message_id: 0,
        agent_type: "rag",
      });
    });

    const registerComposerInsert = vi.fn();

    render(
      <WorkspaceChatPane
        registerComposerInsert={registerComposerInsert}
        selectedSourceIds={["doc-1"]}
        sessionId={null}
        workspaceId="ws-stream-insert"
      />,
    );

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await act(async () => {
      fireEvent.change(composer, { target: { value: "Explain the plan" } });
      fireEvent.keyDown(composer, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    const insertHandler = registerComposerInsert.mock.calls.at(-1)?.[0] as
      | ((text: string) => boolean)
      | null;
    expect(insertHandler?.("INSERT")).toBe(false);
    expect((composer as HTMLTextAreaElement).value).toBe("");
  });

  it("inserts text at the composer cursor when not streaming", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    const registerComposerInsert = vi.fn();

    render(
      <WorkspaceChatPane
        registerComposerInsert={registerComposerInsert}
        selectedSourceIds={[]}
        sessionId={null}
        workspaceId="ws-insert"
      />,
    );

    const composer = await screen.findByRole("textbox", { name: "工作区对话输入框" });
    const insertHandler = registerComposerInsert.mock.calls.at(-1)?.[0] as
      | ((text: string) => boolean)
      | null;
    expect(insertHandler).toBeTypeOf("function");

    fireEvent.change(composer, { target: { value: "hello world" } });
    (composer as HTMLTextAreaElement).setSelectionRange(5, 5);
    expect(insertHandler?.("INSERT")).toBe(true);

    await waitFor(() => {
      expect((composer as HTMLTextAreaElement).value).toBe("helloINSERT world");
    });
  });
});
