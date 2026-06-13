import { render, screen, waitFor } from "@testing-library/react";
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

beforeEach(() => {
  resetWorkspaceChatPaneMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceChatPane transcript", () => {
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
        event: "done",
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

    expect(await screen.findByText("Plan updated")).toBeTruthy();
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
});
