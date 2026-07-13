import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import "./workspace-chat-pane.shared-mocks";

import {
  flushChatPaneMicrotasks,
  renderStreamingChatPane,
  setupWorkspaceChatPaneTestLifecycle,
  submitChatMessage,
  workspaceChatPaneMocks as mocks,
} from "./helpers/workspace-chat-pane.harness";

setupWorkspaceChatPaneTestLifecycle();

describe("WorkspaceChatPane streaming typewriter done finalization", () => {
  it("typewriters a done-only answer (no mid-stream tokens) then finalizes", async () => {
    vi.useFakeTimers();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        event: "done",
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

    const { composer } = renderStreamingChatPane({ workspaceId: "ws-done-only" });
    await submitChatMessage(composer, "Explain the plan");
    await flushChatPaneMicrotasks();

    // Short answers that arrive only on `done` still go through the typewriter queue.
    await vi.runAllTimersAsync();
    await flushChatPaneMicrotasks();

    expect(screen.getByText("Done-only answer")).toBeTruthy();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("finalizes long done answers instead of draining the typewriter queue", async () => {
    vi.useFakeTimers();
    const longAnswer = ["Start", ...Array.from({ length: 8 }, () => "long answer segment")].join(" ");

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        event: "answer_start",
        request_id: "req-long-done",
        session_id: "sess-long-done",
        message_id: 0,
        agent_type: "rag",
      });

      await onEvent({
        event: "token",
        request_id: "req-long-done",
        message_id: 0,
        content: "S",
      });

      await onEvent({
        event: "done",
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

    const { composer } = renderStreamingChatPane({ workspaceId: "ws-long-done" });
    await submitChatMessage(composer, "Explain the long plan");
    await flushChatPaneMicrotasks();

    expect(screen.getByText(longAnswer)).toBeTruthy();
    // Progress card elapsed timer may still be scheduled until React commits completed/hide.
    await vi.runAllTimersAsync();
    await flushChatPaneMicrotasks();
    expect(vi.getTimerCount()).toBe(0);
  });
});
