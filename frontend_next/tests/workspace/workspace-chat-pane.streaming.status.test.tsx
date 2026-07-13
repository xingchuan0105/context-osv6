import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import {
  renderChatPane,
  rerenderChatPane,
  setupWorkspaceChatPaneTestLifecycle,
  workspaceChatPaneMocks as mocks,
} from "./helpers/workspace-chat-pane.harness";
import "./workspace-chat-pane.shared-mocks";

setupWorkspaceChatPaneTestLifecycle();

describe("WorkspaceChatPane streaming status hints", () => {
  it("does not show the guardrail notice when the report only contains allow results", async () => {
    const user = userEvent.setup();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      await onEvent({
        event: "done",
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

    const { composer } = await renderChatPane({
      workspaceId: "ws-allow",
      selectedSourceIds: [],
    });

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
        workspace_id: "ws-chat",
        session_id: null,
        agent_type: "chat",
        doc_scope: [],
        stream: true,
      });

      await onEvent({
        event: "start",
        request_id: "req-chat",
        session_id: "sess-chat",
      });

      await onEvent({
        event: "answer_start",
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
        event: "token",
        request_id: "req-chat",
        message_id: 0,
        content: "Hi",
      });

      await onEvent({
        event: "done",
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

    const { composer } = await renderChatPane({
      workspaceId: "ws-chat",
      selectedSourceIds: [],
    });

    await user.type(composer, "Give me a quick answer");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(tokenReady).toBe(true);
      const statusHint = screen.getByTestId("workspace-status-hint");
      expect(statusHint.closest("article")).toBeNull();
      // Card-level header (may also appear as step title when card body is open).
      expect(within(statusHint).getAllByText("正在思考").length).toBeGreaterThanOrEqual(1);
      expect(statusHint.getAttribute("data-card-collapsed")).toBe("false");
      expect(within(statusHint).queryByText("即时回答")).toBeNull();
      expect(screen.queryByTestId("workspace-progress-card")).toBeNull();
    });

    releaseToken();

    // All 4 modes keep the progress/status card until stream finalize (not hide on first token).
    await waitFor(() => {
      expect(screen.getAllByText("Hi")).toHaveLength(1);
      const statusHint = screen.getByTestId("workspace-status-hint");
      expect(statusHint.getAttribute("data-progress-state")).toBe("completed");
    });
  });

  it("keeps the RAG progress card when sessionId is assigned mid-stream (parent lift)", async () => {
    const user = userEvent.setup();
    let activityReady = false;
    let releaseToken: () => void = () => {
      throw new Error("rag activity gate was released before it was ready");
    };

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({ messages: [] });
    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, request, onEvent) => {
      expect(request).toMatchObject({
        workspace_id: "ws-rag-progress",
        session_id: null,
        agent_type: "rag",
        stream: true,
      });

      await onEvent({
        event: "start",
        request_id: "req-rag-progress",
        session_id: "sess-rag-new",
      });

      await onEvent({
        event: "activity",
        request_id: "req-rag-progress",
        phase: "retrieve_semantic",
        title: "progress.retrieve_semantic.running",
        detail: null,
        counts: { chunks: 12 },
        sources_preview: [],
        timestamp: "10:00",
      });

      await new Promise<void>((resolve) => {
        activityReady = true;
        releaseToken = () => resolve();
      });

      await onEvent({
        event: "token",
        request_id: "req-rag-progress",
        message_id: 0,
        content: "模块包括",
      });

      await onEvent({
        event: "done",
        request_id: "req-rag-progress",
        session_id: "sess-rag-new",
        message_id: 3,
        payload: {
          answer: "模块包括",
          answer_blocks: [],
          session_id: "sess-rag-new",
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
        },
      });
    });

    // Simulate workspace surface: lift session id from stream start into props.
    let liftedSessionId: string | null = null;
    const { composer, rerender } = await renderChatPane({
      workspaceId: "ws-rag-progress",
      sessionId: null,
      selectedSourceIds: ["doc-rag"],
      onSessionChange: (sessionId) => {
        liftedSessionId = sessionId;
      },
    });

    await user.type(composer, "方案有哪些主要模块");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(activityReady).toBe(true);
      expect(liftedSessionId).toBe("sess-rag-new");
      const progress = screen.getByTestId("workspace-progress-card");
      expect(within(progress).getByText("知识库检索中")).toBeTruthy();
    });

    // Parent re-renders with the new session id (this used to hide progress + reset transcript).
    rerenderChatPane(rerender, {
      workspaceId: "ws-rag-progress",
      sessionId: "sess-rag-new",
      selectedSourceIds: ["doc-rag"],
      onSessionChange: (sessionId) => {
        liftedSessionId = sessionId;
      },
    });

    await waitFor(() => {
      const progress = screen.getByTestId("workspace-progress-card");
      expect(within(progress).getByText("知识库检索中")).toBeTruthy();
      // User turn still present — not wiped by history reload.
      expect(screen.getByText("方案有哪些主要模块")).toBeTruthy();
    });

    releaseToken();

    await waitFor(() => {
      expect(screen.getByText("模块包括")).toBeTruthy();
      const progress = screen.getByTestId("workspace-progress-card");
      expect(progress.getAttribute("data-progress-state")).toBe("completed");
    });
  });

  it("places a new turn's progress card after the latest user message, not above the previous answer", async () => {
    const user = userEvent.setup();
    let activityReady = false;
    let releaseToken: () => void = () => {
      throw new Error("follow-up gate released early");
    };

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 1,
          session_id: "sess-multi",
          role: "user",
          content: "第一轮问题",
          answer_blocks: [],
          citations: [],
          created_at: "2026-04-17T00:00:00Z",
        },
        {
          id: 2,
          session_id: "sess-multi",
          role: "assistant",
          content: "第一轮回答内容",
          answer_blocks: [],
          agent_id: "rag",
          citations: [],
          created_at: "2026-04-17T00:01:00Z",
        },
      ],
    });

    mocks.streamWorkspaceChatMock.mockImplementation(async (_token, _request, onEvent) => {
      const sid = "sess-multi";
      const req = "req-multi-followup";

      await onEvent({
        event: "start",
        request_id: req,
        session_id: sid,
      });

      await onEvent({
        event: "activity",
        request_id: req,
        phase: "retrieve_semantic",
        title: "progress.retrieve_semantic.running",
        detail: "q2",
        counts: { chunks: 9 },
        sources_preview: [],
        timestamp: "10:00",
      });

      await new Promise<void>((resolve) => {
        activityReady = true;
        releaseToken = () => resolve();
      });

      await onEvent({
        event: "token",
        request_id: req,
        message_id: 11,
        content: "第二轮回答",
      });

      await onEvent({
        event: "done",
        request_id: req,
        session_id: sid,
        message_id: 11,
        payload: {
          answer: "第二轮回答",
          answer_blocks: [],
          session_id: sid,
          agent_type: "rag",
          sources: [],
          citations: [],
          trace: { mode: "rag" },
          degrade_trace: [],
        },
      });
    });

    const { composer } = await renderChatPane({
      workspaceId: "ws-multi",
      sessionId: "sess-multi",
      selectedSourceIds: ["doc-1"],
    });

    // Existing history is loaded; send a follow-up query.
    await waitFor(() => {
      expect(screen.getByText("第一轮回答内容")).toBeTruthy();
    });

    await user.type(composer, "第二轮问题");
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(activityReady).toBe(true);
      const progress = screen.getByTestId("workspace-progress-card");
      expect(within(progress).getByText("知识库检索中")).toBeTruthy();

      const transcript = progress.closest("[aria-label]") ?? progress.parentElement;
      expect(transcript).toBeTruthy();
      const textOrder = (transcript as HTMLElement).textContent ?? "";
      // Progress must sit after the new user turn, not above the previous answer.
      const prevAnswerAt = textOrder.indexOf("第一轮回答内容");
      const newUserAt = textOrder.indexOf("第二轮问题");
      const progressAt = textOrder.indexOf("知识库检索中");
      expect(prevAnswerAt).toBeGreaterThanOrEqual(0);
      expect(newUserAt).toBeGreaterThan(prevAnswerAt);
      expect(progressAt).toBeGreaterThan(newUserAt);
    });

    releaseToken();

    await waitFor(() => {
      expect(screen.getByText("第二轮回答")).toBeTruthy();
    });
  });
});
