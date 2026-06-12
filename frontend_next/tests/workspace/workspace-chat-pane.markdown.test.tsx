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

describe("WorkspaceChatPane markdown", () => {
  it("renders rich markdown for search text answer blocks", async () => {
    const onOpenWebSources = vi.fn();
    const onSelectCitation = vi.fn();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 3,
          session_id: "sess-search-markdown",
          role: "assistant",
          content: "",
          answer_blocks: [
            {
              type: "text",
              text: [
                "### Research summary",
                "",
                "- First finding [[1]]",
                "- Second finding",
                "",
                "| Signal | Value |",
                "| --- | --- |",
                "| Confidence | High |",
                "",
                "```",
                "const value = 1;",
                "```",
              ].join("\n"),
              citations: [],
            },
          ],
          agent_id: "search",
          citations: [
            {
              citation_id: 1,
              doc_id: "https://source.example/research",
              doc_name: "Search Source",
              preview: "Source preview",
              score: 1,
              source_locator: { url: "https://source.example/research" },
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-search-markdown"
        sessionId="sess-search-markdown"
        selectedSourceIds={[]}
        onOpenWebSources={onOpenWebSources}
        onSelectCitation={onSelectCitation}
      />,
    );

    expect(await screen.findByRole("heading", { name: "Research summary" })).toBeTruthy();
    const firstFinding = screen.getByText("First finding");
    expect(firstFinding.closest("li")).toBeTruthy();
    expect(screen.getByText("Signal").tagName.toLowerCase()).toBe("th");
    expect(screen.getByText("Confidence").tagName.toLowerCase()).toBe("td");
    expect(screen.getByText("High").tagName.toLowerCase()).toBe("td");
    expect(screen.getByText("const value = 1;").tagName.toLowerCase()).toBe("code");

    await userEvent.click(screen.getByRole("button", { name: "引用 1：Search Source" }));
    expect(onOpenWebSources).toHaveBeenCalledWith({
      sources: [
        {
          title: "Search Source",
          url: "https://source.example/research",
          snippet: "Source preview",
        },
      ],
    });
    expect(onSelectCitation).not.toHaveBeenCalled();
  });

  it("renders rich markdown for general and rag text answer blocks", async () => {
    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 4,
          session_id: "sess-global-markdown",
          role: "assistant",
          content: "",
          answer_blocks: [
            {
              type: "text",
              text: [
                "## Chat mode summary",
                "",
                "1. **Ready**",
                "2. Stable",
                "",
                "```json",
                "{\"ok\":true}",
                "```",
              ].join("\n"),
              citations: [],
            },
          ],
          agent_id: "general",
          citations: [],
          created_at: "2026-04-17T00:02:00Z",
        },
        {
          id: 5,
          session_id: "sess-global-markdown",
          role: "assistant",
          content: "",
          answer_blocks: [
            {
              type: "text",
              text: [
                "### RAG mode summary",
                "",
                "- Evidence can still render as markdown",
                "",
                "| Mode | Rendered |",
                "| --- | --- |",
                "| RAG | Yes |",
              ].join("\n"),
              citations: [],
            },
          ],
          agent_id: "rag",
          citations: [],
          created_at: "2026-04-17T00:03:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-global-markdown"
        sessionId="sess-global-markdown"
        selectedSourceIds={["doc-1"]}
      />,
    );

    expect(await screen.findByRole("heading", { name: "Chat mode summary" })).toBeTruthy();
    expect(screen.getByText("Ready").closest("li")).toBeTruthy();
    expect(screen.getByText("{\"ok\":true}").tagName.toLowerCase()).toBe("code");

    expect(screen.getByRole("heading", { name: "RAG mode summary" })).toBeTruthy();
    expect(screen.getByText("Evidence can still render as markdown").closest("li")).toBeTruthy();
    expect(screen.getByText("Mode").tagName.toLowerCase()).toBe("th");
    expect(screen.getAllByText("RAG").some((node) => node.tagName.toLowerCase() === "td")).toBe(true);
  });

  it("opens collected web sources for search citations without changing rag inline citations", async () => {
    const onOpenWebSources = vi.fn();
    const onSelectCitation = vi.fn();

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 4,
          session_id: "sess-web-sources",
          role: "assistant",
          content: "Search answer",
          answer_blocks: [],
          agent_id: "search",
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-with-locator",
              doc_name: "Locator Source",
              preview: "Locator preview",
              score: 0.91,
              source_locator: { url: "https://locator.example/source" },
            },
            {
              citation_id: 2,
              doc_id: "https://fallback.example/doc",
              doc_name: "Fallback Source",
              preview: "Fallback preview",
              score: 0.88,
            },
            {
              citation_id: 3,
              doc_id: "doc-without-url",
              doc_name: "Ignored Source",
              preview: "Ignored preview",
              score: 0.7,
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
        {
          id: 5,
          session_id: "sess-web-sources",
          role: "assistant",
          content: "RAG answer [[1]]",
          answer_blocks: [{ type: "text", text: "RAG answer", citations: ["chunk-rag"] }],
          agent_id: "rag",
          citations: [
            {
              citation_id: 1,
              doc_id: "doc-rag",
              chunk_id: "chunk-rag",
              doc_name: "RAG Doc",
              preview: "RAG preview",
              score: 0.94,
              source_locator: { url: "https://rag.example/source" },
            },
          ],
          created_at: "2026-04-17T00:02:00Z",
        },
      ],
    });

    render(
      <WorkspaceChatPane
        workspaceId="ws-web-sources"
        sessionId="sess-web-sources"
        selectedSourceIds={["doc-rag"]}
        onOpenWebSources={onOpenWebSources}
        onSelectCitation={onSelectCitation}
      />,
    );

    await userEvent.click(await screen.findByRole("button", { name: "2 个来源" }));

    expect(onOpenWebSources).toHaveBeenCalledWith({
      sources: [
        {
          title: "Locator Source",
          url: "https://locator.example/source",
          snippet: "Locator preview",
        },
        {
          title: "Fallback Source",
          url: "https://fallback.example/doc",
          snippet: "Fallback preview",
        },
      ],
    });

    await userEvent.click(screen.getByRole("button", { name: "引用 1：RAG Doc" }));

    expect(onSelectCitation).toHaveBeenCalledWith({
      session_id: "sess-web-sources",
      message_id: 5,
      citation: expect.objectContaining({
        citation_id: 1,
        doc_id: "doc-rag",
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
    expect(onOpenWebSources).toHaveBeenCalledTimes(1);
  });
});
