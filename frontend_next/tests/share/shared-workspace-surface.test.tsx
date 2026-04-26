import type { AnchorHTMLAttributes } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getSharedWorkspaceMock: vi.fn(),
  streamSharedChatMock: vi.fn(),
}));

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
  }),
}));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual("../../lib/share/client");

  return {
    ...actual,
    getSharedWorkspace: mocks.getSharedWorkspaceMock,
    streamSharedChat: mocks.streamSharedChatMock,
  };
});

import { SharedWorkspaceSurface } from "../../components/share/shared-workspace-surface";

function buildPayload(overrides?: Partial<Awaited<ReturnType<typeof mocks.getSharedWorkspaceMock>>>) {
  return {
    knowledge_base: {
      id: "kb-1",
      title: "Shared KB",
      description: "Shared description",
    },
    share: {
      permission: "partial",
      expires_at: "2026-04-30T18:00:00Z",
      allow_download: false,
      scope: "partial",
    },
    sources: [
      {
        id: "src-1",
        file_name: "Plan.pdf",
        status: "ready",
      },
      {
        id: "src-2",
        file_name: "Appendix.txt",
        status: "processing",
      },
    ],
    ...overrides,
  };
}

describe("SharedWorkspaceSurface", () => {
  beforeEach(() => {
    mocks.getSharedWorkspaceMock.mockReset();
    mocks.streamSharedChatMock.mockReset();
  });

  it("shows the loading state while the shared payload is pending", () => {
    mocks.getSharedWorkspaceMock.mockImplementation(() => new Promise(() => undefined));

    render(<SharedWorkspaceSurface shareToken="share-loading" />);

    expect(screen.getByText("正在加载共享内容...")).toBeTruthy();
  });

  it("renders the invalid link state without calling the share client for an empty token", async () => {
    render(<SharedWorkspaceSurface shareToken="" />);

    expect(await screen.findByText("共享链接不可用")).toBeTruthy();
    expect(screen.getByText("这个共享链接无效、已撤销，或已经过期。")).toBeTruthy();
    expect(screen.getByText("invalid")).toBeTruthy();
    expect(mocks.getSharedWorkspaceMock).not.toHaveBeenCalled();
  });

  it("renders partial semantics, download policy, and prompt suggestions", async () => {
    const user = userEvent.setup();
    mocks.getSharedWorkspaceMock.mockResolvedValue(buildPayload());

    render(<SharedWorkspaceSurface shareToken="share-partial" />);

    expect(await screen.findByText("permission=partial")).toBeTruthy();
    expect(screen.getAllByText("partial").length).toBeGreaterThan(0);
    expect(screen.getByText("scope=partial")).toBeTruthy();
    expect(screen.getByText("仅在线查看")).toBeTruthy();
    expect(screen.getByText("allow_download=false")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Shared KB?" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Plan.pdf?" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Plan.pdf?" }));

    expect((screen.getByLabelText("提问") as HTMLTextAreaElement).value).toBe("Plan.pdf?");
  });

  it("renders full semantics and allow_download when the share has full access", async () => {
    mocks.getSharedWorkspaceMock.mockResolvedValue(
      buildPayload({
        share: {
          permission: "full",
          expires_at: null,
          allow_download: true,
          scope: "full",
        },
      }),
    );

    render(<SharedWorkspaceSurface shareToken="share-full" />);

    expect(await screen.findByText("permission=full")).toBeTruthy();
    expect(screen.getByText("permission=full")).toBeTruthy();
    expect(screen.getByText("scope=full")).toBeTruthy();
    expect(screen.getByText("允许下载")).toBeTruthy();
    expect(screen.getByText("allow_download=true")).toBeTruthy();
  });

  it("renders SSE token, citations, done payload, source blocks, and degraded banner", async () => {
    const user = userEvent.setup();
    let releaseDone: () => void = () => {
      throw new Error("Expected the streaming done handler to be registered.");
    };

    mocks.getSharedWorkspaceMock.mockResolvedValue(buildPayload());
    mocks.streamSharedChatMock.mockImplementation(
      async (
        shareToken: string,
        notebookId: string,
        query: string,
        onEvent: (event: any) => void,
      ) => {
        expect(shareToken).toBe("share-stream");
        expect(notebookId).toBe("kb-1");
        expect(query).toBe("What changed?");

        onEvent({
          kind: "token",
          request_id: "req-1",
          message_id: 1,
          content: "Draft answer",
        });
        onEvent({
          kind: "citations",
          request_id: "req-1",
          message_id: 1,
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-1",
              chunk_id: "chunk-1",
              page: 3,
              doc_name: "Plan.pdf",
              preview: "Key excerpt",
              score: 0.9,
            },
          ],
        });

        await new Promise<void>((resolve) => {
          releaseDone = () => {
            onEvent({
              kind: "done",
              request_id: "req-1",
              session_id: "session-1",
              message_id: 1,
              payload: {
                answer: "Final answer",
                answer_blocks: [],
                session_id: "session-1",
                agent_type: "rag",
                sources: [
                  {
                    id: "source-1",
                    title: "Plan.pdf",
                    snippet: "Retrieved source snippet",
                    doc_id: "doc-1",
                    page: 3,
                  },
                ],
                citations: [
                  {
                    citation_id: 1,
                    doc_id: "doc-1",
                    chunk_id: "chunk-1",
                    page: 3,
                    doc_name: "Plan.pdf",
                    preview: "Key excerpt",
                    score: 0.9,
                  },
                ],
                trace: {
                  mode: "rag",
                },
                degrade_trace: [
                  {
                    stage: "retrieval",
                    reason: "partial_index",
                    impact: "lower_recall",
                  },
                ],
              },
            });
            resolve();
          };
        });
      },
    );

    render(<SharedWorkspaceSurface shareToken="share-stream" />);

    await screen.findByText("permission=partial");
    await user.type(screen.getByLabelText("提问"), "What changed?");
    await user.click(screen.getByRole("button", { name: "开始提问" }));

    expect(await screen.findByText("Draft answer")).toBeTruthy();
    expect(screen.getByText("引用资料")).toBeTruthy();
    expect(screen.getAllByText("Key excerpt").length).toBeGreaterThan(0);

    releaseDone();

    await waitFor(() => {
      expect(screen.getByText("Final answer")).toBeTruthy();
    });

    expect(screen.getByText("回答经过降级处理。")).toBeTruthy();
    expect(screen.getByText("partial_index")).toBeTruthy();
    expect(screen.getByText("Retrieved source snippet")).toBeTruthy();
  });

  it("renders SSE error messages without dropping the loaded share overview", async () => {
    const user = userEvent.setup();

    mocks.getSharedWorkspaceMock.mockResolvedValue(buildPayload());
    mocks.streamSharedChatMock.mockImplementation(async (_shareToken, _notebookId, _query, onEvent) => {
      onEvent({
        kind: "error",
        request_id: "req-2",
        code: "stream_failed",
        message: "share stream failed",
      });
    });

    render(<SharedWorkspaceSurface shareToken="share-error" />);

    await screen.findByText("permission=partial");
    await user.type(screen.getByLabelText("提问"), "Need error");
    await user.click(screen.getByRole("button", { name: "开始提问" }));

    expect(await screen.findByText("share stream failed")).toBeTruthy();
    expect(screen.getByText("permission=partial")).toBeTruthy();
  });
});
