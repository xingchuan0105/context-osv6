import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "./workspace-chat-pane.shared-mocks";

import { WorkspaceChatPane } from "../../components/workspace/workspace-chat-pane";

import { mockReducedMotionPreference } from "./helpers/workspace-chat-pane.setup";
import {
  setupWorkspaceChatPaneTestLifecycle,
  workspaceChatPaneMocks as mocks,
} from "./helpers/workspace-chat-pane.harness";

setupWorkspaceChatPaneTestLifecycle();

describe("WorkspaceChatPane streaming typewriter", () => {
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
});
