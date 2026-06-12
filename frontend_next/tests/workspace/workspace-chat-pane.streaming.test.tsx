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

vi.mock("../../lib/workspace/stream", () => ({
  streamWorkspaceChat: mocks.streamWorkspaceChatMock,
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

describe("WorkspaceChatPane streaming", () => {
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
});
