import type { AnchorHTMLAttributes } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const getSharedWorkspaceMock = vi.fn();
const streamChatMock = vi.fn();
const lookupWorkspaceCitationMock = vi.fn();

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
    ...props
  }: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({
    initialized: true,
    token: "token-123",
    user: { id: "u1" },
  }),
}));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual("../../lib/share/client");
  return {
    ...actual,
    getSharedWorkspace: (...args: unknown[]) => getSharedWorkspaceMock(...args),
  };
});

vi.mock("../../lib/runtime/transport", () => ({
  streamChat: (...args: unknown[]) => streamChatMock(...args),
}));

vi.mock("../../lib/workspace/client", async () => {
  const actual = await vi.importActual("../../lib/workspace/client");
  return {
    ...actual,
    lookupWorkspaceCitation: (...args: unknown[]) => lookupWorkspaceCitationMock(...args),
  };
});

import { SharedWorkspaceSurface } from "../../components/share/shared-workspace-surface";

beforeEach(() => {
  window.localStorage.clear();
  getSharedWorkspaceMock.mockReset();
  streamChatMock.mockReset();
  lookupWorkspaceCitationMock.mockReset();
  lookupWorkspaceCitationMock.mockRejectedValue(new Error("share turns are not persisted"));
  getSharedWorkspaceMock.mockResolvedValue({
    knowledge_base: {
      id: "kb-share",
      title: "Shared KB",
      description: "Shared description",
    },
    share: {
      permission: "partial",
      expires_at: null,
      allow_download: false,
      scope: "partial",
    },
    sources: [
      {
        id: "src-1",
        file_name: "Plan.pdf",
        status: "ready",
      },
    ],
  });
});

describe("SharedWorkspaceSurface citations", () => {
  it("opens a citation modal from a streamed share answer without looking up a persisted message", async () => {
    const user = userEvent.setup();
    streamChatMock.mockImplementation(async (_token, request, onEvent) => {
      const sessionId = request.session_id ?? "sess-share";
      await onEvent({
        event: "citations",
        request_id: "req-share",
        message_id: 0,
        citations: [
          {
            citation_id: 1,
            doc_id: "src-1",
            chunk_id: "chunk-1",
            doc_name: "Plan.pdf",
            score: 0.91,
            content: "Local share chunk body",
          },
        ],
      });
      await onEvent({
        event: "done",
        request_id: "req-share",
        session_id: sessionId,
        message_id: 0,
        payload: {
          answer: "Plan updated [[cite:chunk-1]]",
          answer_blocks: [{ type: "text", text: "Plan updated", citations: ["chunk-1"] }],
          session_id: sessionId,
          agent_type: "rag",
          sources: [],
          citations: [
            {
              citation_id: 1,
              doc_id: "src-1",
              chunk_id: "chunk-1",
              doc_name: "Plan.pdf",
              score: 0.91,
            },
          ],
          trace: { mode: "rag" },
          degrade_trace: [],
        },
      });
    });

    render(<SharedWorkspaceSurface shareToken="share-cite" />);

    expect(await screen.findByTestId("shared-chat-pane")).toBeTruthy();
    const composer = screen.getByRole("textbox", { name: "工作区对话输入框" });
    await user.type(composer, "Summarize the plan");
    await user.keyboard("{Enter}");

    expect(await screen.findByText("Plan updated")).toBeTruthy();
    await user.click(await screen.findByRole("button", { name: "引用 1：Plan.pdf" }));

    expect(await screen.findByTestId("workspace-citation-modal")).toBeTruthy();
    expect(screen.getByTestId("workspace-citation-body")).toHaveTextContent("Local share chunk body");
    expect(lookupWorkspaceCitationMock).not.toHaveBeenCalled();
  });
});
