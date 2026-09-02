import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
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

import { ChatCanvas } from "../../components/chat/chat-canvas";

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane capabilities", () => {
  it("auto-attaches rag when sources go from none to selected (2026-08-30 foolproofing)", async () => {
    const requests: Array<{
      agent_type?: string;
      workspace_id?: string;
      doc_scope?: string[];
      capabilities?: string[];
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
      <ChatCanvas
        workspaceId="ws-empty"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const firstComposer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await userEvent.setup().type(firstComposer, "Hello");
    await userEvent.setup().keyboard("{Enter}");

    await waitFor(() => {
      expect(requests[0]).toMatchObject({
        agent_type: "chat",
        capabilities: [],
      });
    });

    firstRender.unmount();

    // Second workspace with a selected source: the first question is grounded —
    // rag auto-attaches without the user touching the chip.
    render(
      <ChatCanvas
        workspaceId="ws-rag"
        sessionId={null}
        selectedSourceIds={["doc-1"]}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
        "true",
      );
    });
    expect(screen.getByTestId("workspace-chat-cap-rag").textContent).toContain("知识库");

    const secondComposer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await userEvent.setup().type(secondComposer, "What is in the doc?");
    await userEvent.setup().keyboard("{Enter}");

    await waitFor(() => {
      expect(requests[1]).toMatchObject({
        agent_type: "rag",
        workspace_id: "ws-rag",
        doc_scope: ["doc-1"],
        capabilities: ["rag"],
      });
    });
  });

  it("treats Shift+Enter as a newline instead of a submit", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(
      <ChatCanvas
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
      <ChatCanvas
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

  it("disables RAG with a guiding mode line when no sources are selected", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    render(
      <ChatCanvas
        workspaceId="ws-noselect"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const ragChip = screen.getByTestId("workspace-chat-cap-rag");
    // Click-to-guide: rag chip stays enabled (clicking opens the right rail), and the
    // mode line names the no-selection state instead of a disabled tooltip.
    expect((ragChip as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByTestId("workspace-chat-mode-line").textContent).toContain("未选择文档");
    // Search stays available.
    expect((screen.getByTestId("workspace-chat-cap-search") as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it("strips RAG when the selection becomes empty", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    // Selected source at mount → rag auto-attaches (2026-08-30 foolproofing).
    const { rerender } = render(
      <ChatCanvas
        workspaceId="ws-strip"
        sessionId={null}
        selectedSourceIds={["doc-1"]}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
        "true",
      );
    });
    // Deselect all sources → RAG is stripped (2026-07-18: no implicit whole-workspace
    // scope); the chip stays enabled as a click-to-guide entry.
    rerender(
      <ChatCanvas
        workspaceId="ws-strip"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
        "false",
      );
    });
    expect(screen.getByTestId("workspace-chat-mode-line").textContent).toContain("未选择文档");
    expect((screen.getByTestId("workspace-chat-cap-rag") as HTMLButtonElement).disabled).toBe(
      false,
    );
  });

  it("does not re-attach rag after the user toggled chips manually", async () => {
    const user = userEvent.setup();
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });

    const { rerender } = render(
      <ChatCanvas
        workspaceId="ws-manual"
        sessionId={null}
        selectedSourceIds={["doc-1"]}
      />,
    );

    // Auto-attach turned rag on for the first observed selection.
    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
        "true",
      );
    });

    // The user explicitly turned it off — that preference must survive.
    await user.click(screen.getByTestId("workspace-chat-cap-rag"));
    expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
      "false",
    );

    // Empty the selection, then pick a different source: no auto re-attach.
    rerender(
      <ChatCanvas
        workspaceId="ws-manual"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-mode-line").textContent).toContain("未选择文档");
    });

    rerender(
      <ChatCanvas
        workspaceId="ws-manual"
        sessionId={null}
        selectedSourceIds={["doc-2"]}
      />,
    );
    // With sources selected again but chips manually turned off, the mode line
    // reports the manual choice — no auto re-attach.
    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-mode-line").textContent).toContain(
        "聊天：回答不检索文档与网络",
      );
    });
    expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
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
      <ChatCanvas
        workspaceId="ws-toggle-caps"
        sessionId={null}
        selectedSourceIds={["doc-1"]}
      />,
    );

    // Auto-attach already turned rag on for the selected source; verify the search
    // toggle composes on top (multiselect), then both chips send rag+search.
    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-cap-rag").getAttribute("aria-pressed")).toBe(
        "true",
      );
    });
    const rag = screen.getByTestId("workspace-chat-cap-rag");
    const search = screen.getByTestId("workspace-chat-cap-search");

    await user.click(search);
    expect(search.getAttribute("aria-pressed")).toBe("true");
    expect(rag.getAttribute("aria-pressed")).toBe("true");

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
      <ChatCanvas
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

  it("keeps personal chat source-free and omits workspace_id from web requests", async () => {
    const user = userEvent.setup();
    const requests: Array<Record<string, unknown>> = [];
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      requests.push(request);
      await onEvent({
        event: "done",
        request_id: "req-personal",
        session_id: "personal-1",
        message_id: 1,
        payload: {
          answer: "web answer",
          answer_blocks: [],
          session_id: "personal-1",
          agent_type: request.agent_type,
          sources: [],
          citations: [],
          trace: { mode: request.agent_type ?? "search" },
          degrade_trace: [],
        },
      });
    });

    render(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    expect(screen.queryByTestId("workspace-chat-cap-rag")).toBeNull();
    expect(screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed")).toBe(
      "false",
    );
    expect(screen.getByTestId("workspace-chat-mode-line").textContent).not.toContain("未选择文档");

    await user.click(screen.getByTestId("workspace-chat-cap-search"));
    await user.type(screen.getByRole("textbox", { name: "对话输入框" }), "Search the web");
    await user.keyboard("{Enter}");

    await waitFor(() => expect(requests).toHaveLength(1));
    expect(requests[0]).toMatchObject({
      agent_type: "search",
      capabilities: ["search"],
      doc_scope: [],
    });
    expect(requests[0]).not.toHaveProperty("workspace_id");
    expect(requests[0]).not.toHaveProperty("session_id");
  });

  it("restores a personal conversation's last Search choice from its transcript", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 1,
          session_id: "personal-history",
          role: "assistant",
          content: "A sourced answer",
          answer_blocks: [],
          citations: [],
          agent_id: "search",
          turn_metadata: { capabilities: ["search"] },
          created_at: "2026-09-02T00:00:00Z",
        },
      ],
    });

    render(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId="personal-history"
        selectedSourceIds={[]}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed"),
      ).toBe("true");
    });
    expect(screen.queryByTestId("workspace-chat-cap-rag")).toBeNull();
  });

  it("carries Search across lazy creation and resets it for the next new chat", async () => {
    const user = userEvent.setup();
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    const { rerender } = render(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    await user.click(screen.getByTestId("workspace-chat-cap-search"));
    expect(screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed")).toBe(
      "true",
    );

    rerender(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId="personal-created"
        selectedSourceIds={[]}
      />,
    );
    await waitFor(() => {
      expect(
        screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed"),
      ).toBe("true");
    });

    rerender(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );
    await waitFor(() => {
      expect(
        screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed"),
      ).toBe("false");
    });
  });

  it("uses resetEpoch to clear Search and draft while already on the new-chat route", async () => {
    const user = userEvent.setup();
    const { rerender } = render(
      <ChatCanvas
        availableCapabilities={["search"]}
        resetEpoch={0}
        workspaceId={null}
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    await user.click(screen.getByTestId("workspace-chat-cap-search"));
    await user.type(screen.getByRole("textbox", { name: "对话输入框" }), "unsent draft");

    rerender(
      <ChatCanvas
        availableCapabilities={["search"]}
        resetEpoch={1}
        workspaceId={null}
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed")).toBe(
        "false",
      );
      expect((screen.getByRole("textbox", { name: "对话输入框" }) as HTMLTextAreaElement).value).toBe(
        "",
      );
    });
  });

  it("disables the Composer while history hydrates and re-enables it after success", async () => {
    let resolveHistory!: (value: { messages: [] }) => void;
    mocks.listWorkspaceSessionMessagesMock.mockReturnValue(
      new Promise((resolve) => {
        resolveHistory = resolve;
      }),
    );

    render(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId="personal-hydrating"
        selectedSourceIds={[]}
      />,
    );

    const composer = screen.getByRole("textbox", { name: "对话输入框" });
    expect(composer).toBeDisabled();
    expect(screen.getByTestId("workspace-chat-cap-search")).toBeDisabled();

    await act(async () => {
      resolveHistory({ messages: [] });
    });

    await waitFor(() => {
      expect(composer).not.toBeDisabled();
      expect(screen.getByTestId("workspace-chat-cap-search")).not.toBeDisabled();
    });
  });

  it("keeps the Composer disabled when history hydration fails", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockRejectedValue(new Error("history unavailable"));

    render(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId="personal-history-error"
        selectedSourceIds={[]}
      />,
    );

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "对话输入框" })).toBeDisabled();
    expect(screen.getByTestId("workspace-chat-cap-search")).toBeDisabled();
  });

  it("does not abort when the first stream start lifts its own assigned session id", async () => {
    const user = userEvent.setup();
    let streamSignal: AbortSignal | null = null;
    mocks.streamWorkspaceChatMock.mockImplementation(
      async (_token, _request, onEvent, options) => {
        streamSignal = options.signal;
        await onEvent({
          event: "start",
          request_id: "req-start",
          session_id: "personal-assigned",
        });
        await new Promise(() => {});
      },
    );

    function AssignedSessionHarness() {
      const [sessionId, setSessionId] = useState<string | null>(null);
      return (
        <ChatCanvas
          availableCapabilities={["search"]}
          workspaceId={null}
          sessionId={sessionId}
          selectedSourceIds={[]}
          onSessionChange={setSessionId}
        />
      );
    }

    const view = render(<AssignedSessionHarness />);
    await user.type(screen.getByRole("textbox", { name: "对话输入框" }), "hello");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(streamSignal).not.toBeNull();
      expect(streamSignal?.aborted).toBe(false);
    });
    view.unmount();
  });

  it("aborts an in-flight stream when an external session replaces it", async () => {
    const user = userEvent.setup();
    let streamSignal: AbortSignal | null = null;
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(
      async (_token, _request, _onEvent, options) => {
        streamSignal = options.signal;
        await new Promise(() => {});
      },
    );

    const view = render(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId="personal-a"
        selectedSourceIds={[]}
      />,
    );
    const composer = screen.getByRole("textbox", { name: "对话输入框" });
    await waitFor(() => expect(composer).not.toBeDisabled());
    await user.type(composer, "first session request");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(streamSignal).not.toBeNull());

    view.rerender(
      <ChatCanvas
        availableCapabilities={["search"]}
        workspaceId={null}
        sessionId="personal-b"
        selectedSourceIds={[]}
      />,
    );

    await waitFor(() => expect(streamSignal?.aborted).toBe(true));
  });
});
