import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { workspaceUiStore } from "../../lib/workspace/ui-store";

import {
  renderChatPane,
  setupWorkspaceChatPaneTestLifecycle,
  workspaceChatPaneMocks as mocks,
} from "./helpers/workspace-chat-pane.harness";
import "./workspace-chat-pane.shared-mocks";

setupWorkspaceChatPaneTestLifecycle();

describe("WorkspaceChatPane streaming search flow", () => {
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

    const { composer } = await renderChatPane({
      selectedSourceIds: ["doc-1", "doc-2"],
      onFocusSource,
      onSelectCitation,
      onSessionActivity,
      onSessionChange,
    });
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
});
