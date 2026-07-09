import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import {
  renderChatPane,
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
