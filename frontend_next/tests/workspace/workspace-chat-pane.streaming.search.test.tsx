import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  renderChatPane,
  setupWorkspaceChatPaneTestLifecycle,
  workspaceChatPaneMocks as mocks,
} from "./helpers/workspace-chat-pane.harness";
import "./workspace-chat-pane.shared-mocks";

setupWorkspaceChatPaneTestLifecycle();

describe("WorkspaceChatPane streaming search flow", () => {
  it("streams assistant tokens incrementally after enabling the Search capability", async () => {
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
        workspace_id: "ws-1",
        session_id: null,
        agent_type: "search",
        doc_scope: ["doc-1", "doc-2"],
        stream: true,
      });

      await onEvent({
        event: "start",
        request_id: "req-1",
        session_id: "sess-new",
      });

      await onEvent({
        event: "activity",
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
        event: "answer_start",
        request_id: "req-1",
        session_id: "sess-new",
        message_id: 0,
        agent_type: "search",
      });

      await onEvent({
        event: "token",
        request_id: "req-1",
        message_id: 0,
        content: "Hel",
      });

      await new Promise<void>((resolve) => {
        firstTokenReady = true;
        releaseStreamFinish = () => resolve();
      });

      await onEvent({
        event: "token",
        request_id: "req-1",
        message_id: 0,
        content: "lo",
      });

      await onEvent({
        event: "citations",
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
        event: "done",
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
    await user.click(screen.getByTestId("workspace-chat-cap-search"));
    expect(screen.getByTestId("workspace-chat-cap-search").getAttribute("aria-pressed")).toBe(
      "true",
    );

    await user.type(composer, "Explain the plan");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(mocks.streamWorkspaceChatMock).toHaveBeenCalledTimes(1);
    });
    expect(onSessionActivity).toHaveBeenCalledTimes(1);

    await waitFor(() => {
      expect(answerStartReady).toBe(true);
      const statusLine = screen.getByTestId("workspace-progress-status-line");
      expect(statusLine).toBeTruthy();
      expect(statusLine.closest("article")).toBeNull();
      expect(statusLine.getAttribute("data-progress-state")).toBe("live");
      // Single-line indicator: current step title + detail inline — no card,
      // no expandable sections, no step list.
      expect(within(statusLine).getByText("正在搜索网页")).toBeTruthy();
      expect(within(statusLine).getByText("系统正在读取多个网页来源。")).toBeTruthy();
      expect(within(statusLine).queryByRole("button")).toBeNull();
    });

    releaseAnswerStart();

    await waitFor(() => {
      expect(firstTokenReady).toBe(true);
      const statusLine = screen.getByTestId("workspace-progress-status-line");
      expect(within(statusLine).getByText("正在搜索网页")).toBeTruthy();
    });

    releaseStreamFinish();

    await waitFor(() => {
      expect(screen.getByText("Hello")).toBeTruthy();
    });
    expect(screen.getAllByText("Hello")).toHaveLength(1);
    expect(screen.getByText("Hello").closest('[data-testid="workspace-answer-bubble"]')?.getAttribute("data-mode")).toBe("search");

    // End-state: completed and collapsed by default (expand via toggle).
    await waitFor(() => {
      const statusLine = screen.getByTestId("workspace-progress-status-line");
      expect(statusLine.getAttribute("data-progress-state")).toBe("completed");
      expect(statusLine.getAttribute("data-collapsed")).toBe("true");
      expect(within(statusLine).getByText("网络搜索")).toBeTruthy();
      expect(within(statusLine).getByTestId("workspace-progress-collapse-toggle")).toBeTruthy();
      expect(within(statusLine).queryByText("正在搜索网页")).toBeNull();
    });

    expect(onSessionChange).toHaveBeenCalledWith("sess-new");
    await waitFor(() => {
      expect(screen.getByText("安全护栏已介入当前回答。")).toBeTruthy();
      expect(screen.getByText(/回答说明：改为摘要回答/)).toBeTruthy();
    });
    // Internal tool plumbing codes must not surface.
    expect(screen.queryByText(/tool_unavailable/)).toBeNull();

    expect(screen.queryByRole("button", { name: "Doc Two" })).toBeNull();
    expect(onFocusSource).not.toHaveBeenCalled();
    expect(onSelectCitation).not.toHaveBeenCalled();
  });
});
