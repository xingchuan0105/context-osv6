import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listWorkspaceSessionMessagesMock: vi.fn(),
  streamWorkspaceChatMock: vi.fn(),
  useAuthMock: vi.fn(),
}));

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

vi.mock("../../lib/workspace/stream", () => ({
  streamWorkspaceChat: mocks.streamWorkspaceChatMock,
}));

import { WorkspaceChatPane } from "../../components/workspace/workspace-chat-pane";
import { workspaceUiStore } from "../../lib/workspace/ui-store";

function mockReducedMotionPreference(matches: boolean) {
  const originalMatchMedia = window.matchMedia;

  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn((query: string) => ({
      matches: matches && query === "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }) as MediaQueryList),
  });

  return () => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      writable: true,
      value: originalMatchMedia,
    });
  };
}

beforeEach(() => {
  window.localStorage.clear();
  workspaceUiStore.setState((state) => ({ ...state, workspaces: {} }));
  mocks.listWorkspaceSessionMessagesMock.mockReset();
  mocks.streamWorkspaceChatMock.mockReset();
  mocks.useAuthMock.mockReset();
  mocks.useAuthMock.mockReturnValue({
    initialized: true,
    isAuthenticated: true,
    token: "token-123",
    user: { id: "user-1", email: "user@example.com", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  });
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane", () => {
  it("loads the existing transcript and focuses sources from citation clicks", async () => {
    const onFocusSource = vi.fn();
    const onSelectCitation = vi.fn();
    const onSessionActivity = vi.fn();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 1,
          session_id: "sess-1",
          role: "user",
          content: "What changed?",
          answer_blocks: [],
          citations: [],
          created_at: "2026-04-17T00:00:00Z",
        },
        {
          id: 2,
          session_id: "sess-1",
          role: "assistant",
          content: "The plan changed. [[1]]",
          answer_blocks: [{ type: "text", text: "The plan changed.", citations: ["chunk-1"] }],
          agent_id: "rag",
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-1",
              chunk_id: "chunk-1",
              doc_name: "Doc One",
              score: 0.93,
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-1"
        sessionId="sess-1"
        selectedSourceIds={["doc-1"]}
        onFocusSource={onFocusSource}
        onSelectCitation={onSelectCitation}
        onSessionActivity={onSessionActivity}
      />,
    );

    await waitFor(() => {
      expect(mocks.listWorkspaceSessionMessagesMock).toHaveBeenCalledWith("token-123", "sess-1");
    });

    expect(screen.getByText("What changed?")).toBeTruthy();
    expect(screen.getByText("The plan changed.")).toBeTruthy();
    expect(screen.queryByText("[[1]]")).toBeNull();

    const userMessage = screen.getByText("What changed?").closest('[data-testid="chat-message"]');
    const assistantBubble = screen.getByText("The plan changed.").closest('[data-testid="workspace-answer-bubble"]');
    expect(userMessage?.getAttribute("data-role")).toBe("user");
    expect(assistantBubble?.closest('[data-testid="chat-message"]')?.getAttribute("data-role")).toBe("assistant");
    expect(assistantBubble?.getAttribute("data-mode")).toBe("rag");
    expect(assistantBubble?.closest('[data-testid="chat-message"]')?.getAttribute("data-pending")).not.toBe("true");

    const citationButton = screen.getByRole("button", { name: "引用 1：Doc One" });
    await userEvent.click(citationButton);

    expect(onFocusSource).not.toHaveBeenCalled();
    expect(onSelectCitation).toHaveBeenCalledWith({
      session_id: "sess-1",
      message_id: 2,
      citation: expect.objectContaining({
        citation_id: 1,
        doc_id: "doc-1",
      }),
      anchorRect: expect.objectContaining({
        top: expect.any(Number),
        left: expect.any(Number),
        right: expect.any(Number),
        bottom: expect.any(Number),
        width: expect.any(Number),
        height: expect.any(Number),
      }),
    });
    expect(onSessionActivity).not.toHaveBeenCalled();
  });

  it("renders chunk-level inline citations from the streaming done payload", async () => {
    const onFocusSource = vi.fn();
    const onSelectCitation = vi.fn();
    const user = userEvent.setup();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      expect(request).toMatchObject({
        notebook_id: "ws-inline-rag",
        session_id: null,
        agent_type: "rag",
        doc_scope: ["doc-1"],
        stream: true,
      });

      await onEvent({
        kind: "done",
        request_id: "req-inline",
        session_id: "sess-inline",
        message_id: 9,
        payload: {
          answer: "Plan updated [[1]]",
          answer_blocks: [{ type: "text", text: "Plan updated", citations: ["chunk-1"] }],
          session_id: "sess-inline",
          agent_type: "rag",
          sources: [],
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-1",
              chunk_id: "chunk-1",
              page: 3,
              doc_name: "Doc One",
              score: 0.94,
            },
          ],
          trace: { mode: "rag" },
          degrade_trace: [],
        },
      });
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-inline-rag"
        sessionId={null}
        selectedSourceIds={["doc-1"]}
        onFocusSource={onFocusSource}
        onSelectCitation={onSelectCitation}
      />,
    );

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(composer, "Summarize the plan");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(screen.getByText("Plan updated")).toBeTruthy();
    });
    expect(screen.queryByText("[[1]]")).toBeNull();

    const inlineCitation = await screen.findByRole("button", { name: "引用 1：Doc One，第 3 页" });
    await user.click(inlineCitation);

    expect(onFocusSource).not.toHaveBeenCalled();
    expect(onSelectCitation).toHaveBeenCalledWith({
      session_id: "sess-inline",
      message_id: 9,
      citation: expect.objectContaining({
        citation_id: 1,
        chunk_id: "chunk-1",
      }),
      anchorRect: expect.objectContaining({
        top: expect.any(Number),
        left: expect.any(Number),
        right: expect.any(Number),
        bottom: expect.any(Number),
        width: expect.any(Number),
        height: expect.any(Number),
      }),
    });
  });

  it("renders rich markdown for search text answer blocks", async () => {
    const onOpenWebSources = vi.fn();
    const onSelectCitation = vi.fn();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 3,
          session_id: "sess-search-markdown",
          role: "assistant",
          content: "",
          answer_blocks: [
            {
              type: "text",
              text: [
                "### Research summary",
                "",
                "- First finding [[1]]",
                "- Second finding",
                "",
                "| Signal | Value |",
                "| --- | --- |",
                "| Confidence | High |",
                "",
                "```",
                "const value = 1;",
                "```",
              ].join("\n"),
              citations: [],
            },
          ],
          agent_id: "search",
          citations: [
            {
              citation_id: 1,
              doc_id: "https://source.example/research",
              doc_name: "Search Source",
              preview: "Source preview",
              score: 1,
              source_locator: { url: "https://source.example/research" },
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-search-markdown"
        sessionId="sess-search-markdown"
        selectedSourceIds={[]}
        onOpenWebSources={onOpenWebSources}
        onSelectCitation={onSelectCitation}
      />,
    );

    expect(await screen.findByRole("heading", { name: "Research summary" })).toBeTruthy();
    const firstFinding = screen.getByText("First finding");
    expect(firstFinding.closest("li")).toBeTruthy();
    expect(screen.getByText("Signal").tagName.toLowerCase()).toBe("th");
    expect(screen.getByText("Confidence").tagName.toLowerCase()).toBe("td");
    expect(screen.getByText("High").tagName.toLowerCase()).toBe("td");
    expect(screen.getByText("const value = 1;").tagName.toLowerCase()).toBe("code");

    await userEvent.click(screen.getByRole("button", { name: "引用 1：Search Source" }));
    expect(onOpenWebSources).toHaveBeenCalledWith({
      sources: [
        {
          title: "Search Source",
          url: "https://source.example/research",
          snippet: "Source preview",
        },
      ],
    });
    expect(onSelectCitation).not.toHaveBeenCalled();
  });

  it("renders rich markdown for general and rag text answer blocks", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 4,
          session_id: "sess-global-markdown",
          role: "assistant",
          content: "",
          answer_blocks: [
            {
              type: "text",
              text: [
                "## Chat mode summary",
                "",
                "1. **Ready**",
                "2. Stable",
                "",
                "```json",
                "{\"ok\":true}",
                "```",
              ].join("\n"),
              citations: [],
            },
          ],
          agent_id: "general",
          citations: [],
          created_at: "2026-04-17T00:02:00Z",
        },
        {
          id: 5,
          session_id: "sess-global-markdown",
          role: "assistant",
          content: "",
          answer_blocks: [
            {
              type: "text",
              text: [
                "### RAG mode summary",
                "",
                "- Evidence can still render as markdown",
                "",
                "| Mode | Rendered |",
                "| --- | --- |",
                "| RAG | Yes |",
              ].join("\n"),
              citations: [],
            },
          ],
          agent_id: "rag",
          citations: [],
          created_at: "2026-04-17T00:03:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-global-markdown"
        sessionId="sess-global-markdown"
        selectedSourceIds={["doc-1"]}
      />,
    );

    expect(await screen.findByRole("heading", { name: "Chat mode summary" })).toBeTruthy();
    expect(screen.getByText("Ready").closest("li")).toBeTruthy();
    expect(screen.getByText("{\"ok\":true}").tagName.toLowerCase()).toBe("code");

    expect(screen.getByRole("heading", { name: "RAG mode summary" })).toBeTruthy();
    expect(screen.getByText("Evidence can still render as markdown").closest("li")).toBeTruthy();
    expect(screen.getByText("Mode").tagName.toLowerCase()).toBe("th");
    expect(screen.getAllByText("RAG").some((node) => node.tagName.toLowerCase() === "td")).toBe(true);
  });

  it("opens collected web sources for search citations without changing rag inline citations", async () => {
    const onOpenWebSources = vi.fn();
    const onSelectCitation = vi.fn();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 4,
          session_id: "sess-web-sources",
          role: "assistant",
          content: "Search answer",
          answer_blocks: [],
          agent_id: "search",
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-with-locator",
              doc_name: "Locator Source",
              preview: "Locator preview",
              score: 0.91,
              source_locator: { url: "https://locator.example/source" },
            },
            {
              citation_id: 2,
              doc_id: "https://fallback.example/doc",
              doc_name: "Fallback Source",
              preview: "Fallback preview",
              score: 0.88,
            },
            {
              citation_id: 3,
              doc_id: "doc-without-url",
              doc_name: "Ignored Source",
              preview: "Ignored preview",
              score: 0.7,
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
        {
          id: 5,
          session_id: "sess-web-sources",
          role: "assistant",
          content: "RAG answer [[1]]",
          answer_blocks: [{ type: "text", text: "RAG answer", citations: ["chunk-rag"] }],
          agent_id: "rag",
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-rag",
              chunk_id: "chunk-rag",
              doc_name: "RAG Doc",
              preview: "RAG preview",
              score: 0.94,
              source_locator: { url: "https://rag.example/source" },
            },
          ],
          created_at: "2026-04-17T00:02:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-web-sources"
        sessionId="sess-web-sources"
        selectedSourceIds={["doc-rag"]}
        onOpenWebSources={onOpenWebSources}
        onSelectCitation={onSelectCitation}
      />,
    );

    await userEvent.click(await screen.findByRole("button", { name: "2 个来源" }));

    expect(onOpenWebSources).toHaveBeenCalledWith({
      sources: [
        {
          title: "Locator Source",
          url: "https://locator.example/source",
          snippet: "Locator preview",
        },
        {
          title: "Fallback Source",
          url: "https://fallback.example/doc",
          snippet: "Fallback preview",
        },
      ],
    });

    await userEvent.click(screen.getByRole("button", { name: "引用 1：RAG Doc" }));

    expect(onSelectCitation).toHaveBeenCalledWith({
      session_id: "sess-web-sources",
      message_id: 5,
      citation: expect.objectContaining({
        citation_id: 1,
        doc_id: "doc-rag",
      }),
      anchorRect: expect.objectContaining({
        top: expect.any(Number),
        left: expect.any(Number),
        right: expect.any(Number),
        bottom: expect.any(Number),
        width: expect.any(Number),
        height: expect.any(Number),
      }),
    });
    expect(onOpenWebSources).toHaveBeenCalledTimes(1);
  });

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

  it("streams assistant tokens incrementally, supports '/' mode selection, and persists the mode in the workspace UI store", async () => {
    const onFocusSource = vi.fn();
    const onSelectCitation = vi.fn();
    const onSessionActivity = vi.fn();
    const onSessionChange = vi.fn();
    const user = userEvent.setup();
    let answerStartReady = false;
    let firstTokenReady = false;
    let releaseAnswerStart: () => void = () => {
      throw new Error("search answer gate was released before it was ready");
    };
    let releaseStreamFinish: () => void = () => {
      throw new Error("search stream finish gate was released before it was ready");
    };

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      expect(request).toMatchObject({
        notebook_id: "ws-1",
        session_id: null,
        agent_type: "search",
        doc_scope: ["doc-1", "doc-2"],
        stream: true,
      });

      await onEvent({
        kind: "start",
        request_id: "req-1",
        session_id: "sess-new",
      });

      await onEvent({
        kind: "activity",
        request_id: "req-1",
        phase: "searching",
        title: "正在搜索网页",
        detail: "系统正在读取多个网页来源。",
        counts: {
          queries: 2,
          sources: 4,
        },
        sources_preview: [
          {
            id: "source-1",
            label: "example.com",
          },
        ],
        timestamp: "10:00",
      });

      await new Promise<void>((resolve) => {
        answerStartReady = true;
        releaseAnswerStart = () => resolve();
      });

      await onEvent({
        kind: "answer_start",
        request_id: "req-1",
        session_id: "sess-new",
        message_id: 0,
        agent_type: "search",
      });

      await onEvent({
        kind: "token",
        request_id: "req-1",
        message_id: 0,
        content: "Hel",
      });

      await new Promise<void>((resolve) => {
        firstTokenReady = true;
        releaseStreamFinish = () => resolve();
      });

      await onEvent({
        kind: "token",
        request_id: "req-1",
        message_id: 0,
        content: "lo",
      });

      await onEvent({
        kind: "citations",
        request_id: "req-1",
        message_id: 11,
        citations: [
          {
            citation_id: 1,
            doc_id: "doc-2",
            doc_name: "Doc Two",
            score: 0.88,
          },
        ],
      });

      await new Promise((resolve) => setTimeout(resolve, 25));

      await onEvent({
        kind: "done",
        request_id: "req-1",
        session_id: "sess-new",
        message_id: 11,
        payload: {
          answer: "Hello",
          answer_blocks: [],
          session_id: "sess-new",
          agent_type: "search",
          sources: [],
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-2",
              doc_name: "Doc Two",
              score: 0.88,
            },
          ],
          trace: { mode: "search" },
          degrade_trace: [
            {
              stage: "retrieval",
              reason: "fallback_to_summary",
              impact: "partial_context",
            },
          ],
          guard_report: {
            blocked: false,
            output_results: [
              {
                passed: true,
                guard_type: "pii_scrubber",
                risk_level: "Medium",
                action: "Flag",
                reason: "sensitive entity detected",
              },
            ],
          },
        },
      });
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-1"
        sessionId={null}
        selectedSourceIds={["doc-1", "doc-2"]}
        onFocusSource={onFocusSource}
        onSelectCitation={onSelectCitation}
        onSessionActivity={onSessionActivity}
        onSessionChange={onSessionChange}
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "工作区对话输入框" })).toBeTruthy();
    });

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.click(composer);
    await user.keyboard("/");
    await user.click(screen.getByRole("button", { name: /网络搜索\s*web_search/i }));
    expect(workspaceUiStore.getState().workspaces["ws-1"]?.chatMode).toBe("search");
    expect(workspaceUiStore.getState().workspaces["ws-1"]?.chatModePreference).toBe("manual");

    await user.type(composer, "Explain the plan");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(mocks.streamWorkspaceChatMock).toHaveBeenCalledTimes(1);
    });
    expect(onSessionActivity).toHaveBeenCalledTimes(1);

    await waitFor(() => {
      expect(answerStartReady).toBe(true);
      const progressCard = screen.getByTestId("workspace-progress-card");
      expect(progressCard).toBeTruthy();
      expect(progressCard.closest("article")).toBeNull();
      expect(within(progressCard).getByText("网络搜索中")).toBeTruthy();
      expect(within(progressCard).queryByText("正在搜索网页")).toBeNull();
      expect(within(progressCard).queryByText("系统正在读取多个网页来源。")).toBeNull();
    });

    await user.click(screen.getByRole("button", { name: "展开过程" }));
    const expandedProgressCard = screen.getByTestId("workspace-progress-card");
    expect(within(expandedProgressCard).getByText("正在搜索网页")).toBeTruthy();
    expect(within(expandedProgressCard).getByText("系统正在读取多个网页来源。")).toBeTruthy();
    expect(within(expandedProgressCard).getByText("查询 2")).toBeTruthy();
    expect(within(expandedProgressCard).getByText("来源 4")).toBeTruthy();
    expect(within(expandedProgressCard).getByText("example.com")).toBeTruthy();

    releaseAnswerStart();

    await waitFor(() => {
      expect(firstTokenReady).toBe(true);
      const progressCard = screen.getByTestId("workspace-progress-card");
      expect(within(progressCard).getByText("网络搜索中")).toBeTruthy();
      expect(within(progressCard).getByText("正在搜索网页")).toBeTruthy();
    });

    releaseStreamFinish();

    await waitFor(() => {
      expect(screen.getByText("Hello")).toBeTruthy();
    });
    expect(screen.getAllByText("Hello")).toHaveLength(1);
    expect(screen.getByText("Hello").closest('[data-testid="workspace-answer-bubble"]')?.getAttribute("data-mode")).toBe("search");

    expect(onSessionChange).toHaveBeenCalledWith("sess-new");
    await waitFor(() => {
      expect(screen.getByText("Guardrail 已介入当前回答。")).toBeTruthy();
      expect(screen.getByText("降级原因：fallback_to_summary")).toBeTruthy();
    });

    expect(screen.queryByRole("button", { name: "Doc Two" })).toBeNull();
    expect(onFocusSource).not.toHaveBeenCalled();
    expect(onSelectCitation).not.toHaveBeenCalled();
  });

  it("does not type done answer suffix before applying final metadata", async () => {
    vi.useFakeTimers();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        kind: "answer_start",
        request_id: "req-type",
        session_id: "sess-type",
        message_id: 0,
        agent_type: "rag",
      });

      await onEvent({
        kind: "token",
        request_id: "req-type",
        message_id: 0,
        content: "Hel",
      });

      await onEvent({
        kind: "done",
        request_id: "req-type",
        session_id: "sess-type",
        message_id: 21,
        payload: {
          answer: "Hello",
          answer_blocks: [],
          session_id: "sess-type",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [
            {
              stage: "retrieval",
              reason: "fallback_to_summary",
              impact: "partial_context",
            },
          ],
          guard_report: {
            blocked: false,
            output_results: [
              {
                passed: true,
                guard_type: "pii_scrubber",
                risk_level: "Medium",
                action: "Flag",
                reason: "sensitive entity detected",
              },
            ],
          },
        },
      });
    });

    render(<WorkspaceChatPane workspaceId="ws-type" sessionId={null} selectedSourceIds={["doc-1"]} />);

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await act(async () => {
      fireEvent.change(composer, { target: { value: "Explain the plan" } });
      fireEvent.keyDown(composer, { key: "Enter" });
    });

    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.streamWorkspaceChatMock).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Hello")).toBeNull();
    expect(screen.queryByText("Guardrail 已介入当前回答。")).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(16);
    });

    expect(screen.getByText("Hello")).toBeTruthy();
    expect(screen.queryByText("Hel")).toBeNull();
    expect(screen.getByText("Guardrail 已介入当前回答。")).toBeTruthy();
    expect(screen.getByText("降级原因：fallback_to_summary")).toBeTruthy();
  });

  it("finalizes a done-only answer immediately", async () => {
    vi.useFakeTimers();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        kind: "done",
        request_id: "req-done-only",
        session_id: "sess-done-only",
        message_id: 31,
        payload: {
          answer: "Done-only answer",
          answer_blocks: [],
          session_id: "sess-done-only",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
        },
      });
    });

    render(<WorkspaceChatPane workspaceId="ws-done-only" sessionId={null} selectedSourceIds={["doc-1"]} />);

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await act(async () => {
      fireEvent.change(composer, { target: { value: "Explain the plan" } });
      fireEvent.keyDown(composer, { key: "Enter" });
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByText("Done-only answer")).toBeTruthy();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("finalizes long done answers instead of draining the typewriter queue", async () => {
    vi.useFakeTimers();
    const longAnswer = ["Start", ...Array.from({ length: 8 }, () => "long answer segment")].join(" ");

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        kind: "answer_start",
        request_id: "req-long-done",
        session_id: "sess-long-done",
        message_id: 0,
        agent_type: "rag",
      });

      await onEvent({
        kind: "token",
        request_id: "req-long-done",
        message_id: 0,
        content: "S",
      });

      await onEvent({
        kind: "done",
        request_id: "req-long-done",
        session_id: "sess-long-done",
        message_id: 32,
        payload: {
          answer: longAnswer,
          answer_blocks: [],
          session_id: "sess-long-done",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
        },
      });
    });

    render(<WorkspaceChatPane workspaceId="ws-long-done" sessionId={null} selectedSourceIds={["doc-1"]} />);

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await act(async () => {
      fireEvent.change(composer, { target: { value: "Explain the long plan" } });
      fireEvent.keyDown(composer, { key: "Enter" });
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(screen.getByText(longAnswer)).toBeTruthy();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("renders streaming tokens immediately when reduced motion is preferred", async () => {
    vi.useFakeTimers();
    const restoreMatchMedia = mockReducedMotionPreference(true);
    let releaseDone: () => void = () => {
      throw new Error("reduced-motion done gate was released before it was ready");
    };
    const doneReady = new Promise<void>((resolve) => {
      releaseDone = resolve;
    });

    try {
      mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
      mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
        await onEvent({
          kind: "answer_start",
          request_id: "req-reduce",
          session_id: "sess-reduce",
          message_id: 0,
          agent_type: "rag",
        });

        await onEvent({
          kind: "token",
          request_id: "req-reduce",
          message_id: 0,
          content: "Instant token",
        });

        await doneReady;

        await onEvent({
          kind: "done",
          request_id: "req-reduce",
          session_id: "sess-reduce",
          message_id: 33,
          payload: {
            answer: "Instant token",
            answer_blocks: [],
            session_id: "sess-reduce",
            agent_type: "rag",
            sources: [],
            citations: [],
            trace: { mode: "rag" },
            degrade_trace: [],
          },
        });
      });

      render(<WorkspaceChatPane workspaceId="ws-reduce" sessionId={null} selectedSourceIds={["doc-1"]} />);

      const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
      await act(async () => {
        fireEvent.change(composer, { target: { value: "Explain the plan" } });
        fireEvent.keyDown(composer, { key: "Enter" });
      });

      await act(async () => {
        await Promise.resolve();
      });

      expect(screen.getByText("Instant token")).toBeTruthy();
      expect(vi.getTimerCount()).toBe(0);

      await act(async () => {
        releaseDone();
        await Promise.resolve();
      });

      await act(async () => {
        await Promise.resolve();
      });

      expect(screen.getByText("Instant token")).toBeTruthy();
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      restoreMatchMedia();
    }
  });

  it("does not show the guardrail notice when the report only contains allow results", async () => {
    const user = userEvent.setup();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        kind: "done",
        request_id: "req-allow",
        session_id: "sess-allow",
        message_id: 12,
        payload: {
          answer: "Normal answer",
          answer_blocks: [],
          session_id: "sess-allow",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
          guard_report: {
            blocked: false,
            output_results: [
              {
                passed: true,
                guard_type: "citation_provability",
                risk_level: "Low",
                action: "Allow",
                reason: "",
              },
            ],
          },
        },
      });
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-allow"
        sessionId={null}
        selectedSourceIds={[]}
      />,
    );

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(composer, "Why is this normal?");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(screen.getByText("Normal answer")).toBeTruthy();
    });
    expect(screen.queryByText("Guardrail 已介入当前回答。")).toBeNull();
  });

  it("shows a lightweight thinking hint for chat mode until the first token arrives", async () => {
    const user = userEvent.setup();
    let tokenReady = false;
    let releaseToken: () => void = () => {
      throw new Error("chat token gate was released before it was ready");
    };

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      expect(request).toMatchObject({
        notebook_id: "ws-chat",
        session_id: null,
        agent_type: "chat",
        doc_scope: [],
        stream: true,
      });

      await onEvent({
        kind: "start",
        request_id: "req-chat",
        session_id: "sess-chat",
      });

      await onEvent({
        kind: "answer_start",
        request_id: "req-chat",
        session_id: "sess-chat",
        message_id: 0,
        agent_type: "chat",
      });

      await new Promise<void>((resolve) => {
        tokenReady = true;
        releaseToken = () => resolve();
      });

      await onEvent({
        kind: "token",
        request_id: "req-chat",
        message_id: 0,
        content: "Hi",
      });

      await onEvent({
        kind: "done",
        request_id: "req-chat",
        session_id: "sess-chat",
        message_id: 7,
        payload: {
          answer: "Hi",
          answer_blocks: [],
          session_id: "sess-chat",
          agent_type: "chat",
          sources: [],
          citations: [],
          trace: { mode: "general" },
          degrade_trace: [],
        },
      });
    });

    render(<WorkspaceChatPane workspaceId="ws-chat" sessionId={null} selectedSourceIds={[]} />);

    await waitFor(() => {
      expect(screen.getByRole("textbox", { name: "工作区对话输入框" })).toBeTruthy();
    });

    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(composer, "Give me a quick answer");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(tokenReady).toBe(true);
      const statusHint = screen.getByTestId("workspace-status-hint");
      expect(statusHint.closest("article")).toBeNull();
      expect(within(statusHint).getByText("正在思考")).toBeTruthy();
      expect(within(statusHint).queryByText("即时回答")).toBeNull();
      expect(screen.queryByTestId("workspace-progress-card")).toBeNull();
    });

    releaseToken();

    await waitFor(() => {
      expect(screen.queryByTestId("workspace-status-hint")).toBeNull();
    });

    await waitFor(() => {
      expect(screen.getAllByText("Hi")).toHaveLength(1);
    });
  });

  it("uses chat by default without sources and rag by default when sources are available", async () => {
    const user = userEvent.setup();
    const requests: Array<{ agent_type?: string; notebook_id?: string; doc_scope?: string[] }> = [];

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      requests.push(request);

      await onEvent({
        kind: "done",
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
        notebook_id: "ws-rag",
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
});
