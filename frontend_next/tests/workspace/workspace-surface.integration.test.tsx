import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceSurfaceMocks());

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: mocks.pushMock,
    replace: mocks.replaceMock,
    prefetch: vi.fn(),
    refresh: vi.fn(),
    back: vi.fn(),
    forward: vi.fn(),
  }),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({
    locale: "zh-CN" as const,
    theme: "system" as const,
    setLocale: mocks.setLocaleMock,
    setTheme: mocks.setThemeMock,
  }),
}));

vi.mock("../../lib/dashboard/client", () => ({
  createWorkspace: mocks.createWorkspaceMock,
}));

vi.mock("../../lib/dashboard/default-title", () => ({
  getDefaultWorkspaceTitle: mocks.getDefaultWorkspaceTitleMock,
  markDefaultWorkspaceTitleUsed: mocks.markDefaultWorkspaceTitleUsedMock,
}));

vi.mock("../../lib/workspace/client", () => ({
  getWorkspace: mocks.getWorkspaceMock,
  listWorkspaceSessions: mocks.listWorkspaceSessionsMock,
  listWorkspaceSessionMessages: mocks.listWorkspaceSessionMessagesMock,
  createWorkspaceSession: mocks.createWorkspaceSessionMock,
  updateWorkspace: mocks.updateWorkspaceMock,
  updateWorkspaceSession: mocks.updateWorkspaceSessionMock,
  deleteWorkspaceSession: mocks.deleteWorkspaceSessionMock,
  lookupWorkspaceCitation: mocks.lookupWorkspaceCitationMock,
  addWorkspaceSourceUrl: mocks.addWorkspaceSourceUrlMock,
  completeWorkspaceDocumentUpload: mocks.completeWorkspaceDocumentUploadMock,
  createWorkspaceNote: mocks.createWorkspaceNoteMock,
  createWorkspaceDocumentUpload: mocks.createWorkspaceDocumentUploadMock,
  deleteWorkspaceDocument: mocks.deleteWorkspaceDocumentMock,
  deleteWorkspaceNote: mocks.deleteWorkspaceNoteMock,
  getWorkspaceSourceContent: mocks.getWorkspaceSourceContentMock,
  getWorkspaceSourceParsedPreview: mocks.getWorkspaceSourceParsedPreviewMock,
  listWorkspaceNotes: mocks.listWorkspaceNotesMock,
  listWorkspaceSources: mocks.listWorkspaceSourcesMock,
  promoteWorkspaceNote: mocks.promoteWorkspaceNoteMock,
  reindexWorkspaceDocument: mocks.reindexWorkspaceDocumentMock,
  uploadWorkspaceDocumentFile: mocks.uploadWorkspaceDocumentFileMock,
  updateWorkspaceNote: mocks.updateWorkspaceNoteMock,
}));

vi.mock("../../lib/workspace/stream", () => ({
  streamWorkspaceChat: mocks.streamWorkspaceChatMock,
}));

vi.mock("../../lib/billing/featureFlag", () => ({
  isPricingRevampEnabledSSR: () => true,
  isPricingRevampEnabled: vi.fn().mockResolvedValue(true),
  probePricingRevampUsageWindow: mocks.probePricingRevampUsageWindowMock,
}));

import {
  clearSurfaceMatchMediaListeners,
  installSurfaceMatchMedia,
  resetWorkspaceSurfaceMocks,
  setMobileViewport,
} from "./helpers/workspace-surface.setup";
import { renderWorkspaceSurface } from "./helpers/workspace-surface.harness";
import { workspaceUiStore } from "../../lib/workspace/ui-store";

beforeEach(() => {
  installSurfaceMatchMedia();
  resetWorkspaceSurfaceMocks(mocks);
});

afterEach(() => {
  clearSurfaceMatchMediaListeners();
  vi.clearAllMocks();
});

describe("WorkspaceSurface integration", () => {
  it("renders real chat and right-rail panes and wires store-backed selection through the DOM", async () => {
    const user = userEvent.setup();

    workspaceUiStore.getState().setSelectedSourceIds("ws-1", ["src-2"]);
    workspaceUiStore.getState().setFocusedSourceId("ws-1", "src-2");

    renderWorkspaceSurface("ws-1");

    expect(await screen.findByLabelText("工作区标题")).toBeTruthy();
    expect(screen.getByTestId("desktop-history-rail")).toBeTruthy();
    expect(screen.getByTestId("desktop-right-rail")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "工作区对话输入框" })).toBeTruthy();

    const sourcesList = await screen.findByRole("list", { name: "资料列表" });
    const sourceTwoItem = screen.getByText("source-two.pdf").closest("li");
    expect(sourceTwoItem?.className).toContain("listItemFocused");
    expect(within(sourceTwoItem!).getByRole("button", { pressed: true })).toBeTruthy();
    expect(within(sourcesList).getAllByRole("listitem")).toHaveLength(2);

    await user.click(screen.getByRole("button", { name: "新建会话" }));
    expect(mocks.createWorkspaceSessionMock).not.toHaveBeenCalled();

    const history = screen.getByRole("region", { name: "工作区历史" });
    await user.click(within(history).getByText("General thread"));

    await waitFor(() => {
      expect(mocks.listWorkspaceSessionMessagesMock).toHaveBeenCalledWith("token-123", "sess-2");
    });

    await user.click(screen.getByRole("button", { name: "全选" }));
    await waitFor(() => {
      expect(
        within(sourcesList)
          .getAllByRole("listitem")
          .every((item) => within(item).getByRole("button", { pressed: true })),
      ).toBe(true);
    });

    await user.click(screen.getByRole("button", { name: "创建工作区" }));

    await waitFor(() => {
      expect(mocks.markDefaultWorkspaceTitleUsedMock).toHaveBeenCalled();
      expect(mocks.createWorkspaceMock).toHaveBeenCalledWith("token-123", {
        name: "工作区1",
        description: "",
      });
    });
  });

  it("opens a citation chunk modal from the real chat pane without refocusing the right rail", async () => {
    const user = userEvent.setup();

    workspaceUiStore.getState().setFocusedSourceId("ws-1", "src-1");

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 7,
          session_id: "sess-1",
          role: "assistant",
          content: "Answer with citation [[1]]",
          answer_blocks: [{ type: "text", text: "Answer with citation", citations: ["chunk-1"] }],
          agent_id: "rag",
          citations: [
            {
              citation_id: 1,
              doc_id: "src-2",
              chunk_id: "chunk-1",
              doc_name: "Source Two",
              score: 0.9,
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
      ],
    });

    renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");
    await screen.findByText("Answer with citation");

    const focusedItem = screen.getByText("source-one.pdf").closest("li");
    expect(focusedItem?.className).toContain("listItemFocused");

    await user.click(screen.getByRole("button", { name: "引用 1：Source Two" }));

    await waitFor(() => {
      expect(mocks.lookupWorkspaceCitationMock).toHaveBeenCalledWith("token-123", {
        session_id: "sess-1",
        message_id: 7,
        citation_id: 1,
      });
    });

    expect(screen.getByText("source-one.pdf").closest("li")?.className).toContain("listItemFocused");
    const dialog = screen.getByRole("dialog", { name: "引用片段" });
    expect(screen.getByText("Chunk detail for src-2")).toBeTruthy();

    await user.click(dialog.parentElement as HTMLElement);

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "引用片段" })).toBeNull();
    });
    expect(screen.getByText("source-one.pdf").closest("li")?.className).toContain("listItemFocused");
  });

  it("opens web sources in the real right rail and clears the citation modal", async () => {
    const user = userEvent.setup();

    workspaceUiStore.getState().setRightRailOpen("ws-1", false);

    mocks.listWorkspaceSessionMessagesMock.mockResolvedValue({
      messages: [
        {
          id: 7,
          session_id: "sess-1",
          role: "assistant",
          content: "RAG answer [[1]]",
          answer_blocks: [{ type: "text", text: "RAG answer", citations: ["chunk-1"] }],
          agent_id: "rag",
          citations: [
            {
              citation_id: 1,
              doc_id: "src-2",
              chunk_id: "chunk-1",
              doc_name: "Source Two",
              score: 0.9,
            },
          ],
          created_at: "2026-04-17T00:01:00Z",
        },
        {
          id: 8,
          session_id: "sess-1",
          role: "assistant",
          content: "Search answer",
          answer_blocks: [],
          agent_id: "search",
          citations: [
            {
              citation_id: 1,
              doc_id: "https://example.test/search",
              doc_name: "Search Result",
              preview: "Search snippet",
              score: 1,
              source_locator: { url: "https://example.test/search" },
            },
          ],
          created_at: "2026-04-17T00:02:00Z",
        },
      ],
    });

    renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");
    expect(await screen.findByText("RAG answer")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "引用 1：Source Two" }));
    expect(await screen.findByRole("dialog", { name: "引用片段" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "1 个来源" }));

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "引用片段" })).toBeNull();
      expect(workspaceUiStore.getState().workspaces["ws-1"]?.rightRailOpen).toBe(true);
      expect(screen.getByRole("link", { name: "Search Result" }).getAttribute("href")).toBe(
        "https://example.test/search",
      );
      expect(screen.getByText("Search snippet")).toBeTruthy();
    });
  });
});
