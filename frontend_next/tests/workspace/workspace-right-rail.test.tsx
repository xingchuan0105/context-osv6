import React from "react";
import { fireEvent, render, screen, within, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  authState: {
    initialized: true,
    isAuthenticated: true,
    token: "token-123",
    user: { id: "user-1", email: "user@example.test", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  },
  uiPreferencesState: {
    locale: "en" as "en" | "zh-CN",
    theme: "system" as const,
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  },
  addWorkspaceSourceUrlMock: vi.fn(),
  completeWorkspaceDocumentUploadMock: vi.fn(),
  createWorkspaceNoteMock: vi.fn(),
  createWorkspaceDocumentUploadMock: vi.fn(),
  deleteWorkspaceDocumentMock: vi.fn(),
  deleteWorkspaceNoteMock: vi.fn(),
  getWorkspaceSourceContentMock: vi.fn(),
  getWorkspaceSourceParsedPreviewMock: vi.fn(),
  listWorkspaceNotesMock: vi.fn(),
  listWorkspaceSourcesMock: vi.fn(),
  lookupWorkspaceCitationMock: vi.fn(),
  promoteWorkspaceNoteMock: vi.fn(),
  reindexWorkspaceDocumentMock: vi.fn(),
  uploadWorkspaceDocumentFileMock: vi.fn(),
  updateWorkspaceNoteMock: vi.fn(),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => mocks.uiPreferencesState,
}));

vi.mock("../../lib/workspace/client", () => ({
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
  lookupWorkspaceCitation: mocks.lookupWorkspaceCitationMock,
  promoteWorkspaceNote: mocks.promoteWorkspaceNoteMock,
  reindexWorkspaceDocument: mocks.reindexWorkspaceDocumentMock,
  uploadWorkspaceDocumentFile: mocks.uploadWorkspaceDocumentFileMock,
  updateWorkspaceNote: mocks.updateWorkspaceNoteMock,
}));

import { WorkspaceRightRail } from "../../components/workspace/workspace-right-rail";
import { QueryProvider } from "../../lib/query/provider";
import { getWorkspaceUiState, workspaceUiStore } from "../../lib/workspace/ui-store";

function buildSource(overrides: Partial<{ id: string; file_name: string; status: string; title: string }> = {}) {
  return {
    id: overrides.id ?? "src-1",
    workspace_id: "ws-1",
    workspace_name: "Workspace 1",
    title: overrides.title ?? "Source",
    file_name: overrides.file_name ?? "source.pdf",
    status: overrides.status ?? "processing",
  };
}

function buildNote(
  overrides: Partial<{
    id: string;
    title: string;
    content: string;
    preview: string;
    updated_at: string;
    promoted_document_id: string | null;
    promoted_at: string | null;
  }> = {},
) {
  return {
    id: overrides.id ?? "note-1",
    workspace_id: "ws-1",
    title: overrides.title ?? "Note",
    content: overrides.content ?? "Body",
    preview: overrides.preview ?? "Body",
    created_at: "2026-04-17T00:00:00Z",
    updated_at: overrides.updated_at ?? "2026-04-18T00:00:00Z",
    promoted_document_id: overrides.promoted_document_id ?? null,
    promoted_at: overrides.promoted_at ?? null,
  };
}

function Harness({
  activeWebSources = null,
  focusedSourceId,
  onCloseWebSources,
  selectedSourceIds: initialSelectedSourceIds = [],
}: {
  activeWebSources?: Parameters<typeof WorkspaceRightRail>[0]["activeWebSources"];
  focusedSourceId?: string | null;
  onCloseWebSources?: () => void;
  selectedSourceIds?: string[];
}) {
  const [selectedSourceIds, setSelectedSourceIds] = React.useState(initialSelectedSourceIds);

  return (
    <WorkspaceRightRail
      activeWebSources={activeWebSources}
      focusedSourceId={focusedSourceId}
      onCloseWebSources={onCloseWebSources}
      onSelectedSourceIdsChange={setSelectedSourceIds}
      selectedSourceIds={selectedSourceIds}
      workspaceId="ws-1"
    />
  );
}

function renderHarness(
  props: Parameters<typeof Harness>[0] = {},
) {
  return render(
    <QueryProvider>
      <Harness {...props} />
    </QueryProvider>,
  );
}

beforeEach(() => {
  workspaceUiStore.setState((state) => ({ ...state, workspaces: {} }));
  mocks.authState = {
    initialized: true,
    isAuthenticated: true,
    token: "token-123",
    user: { id: "user-1", email: "user@example.test", full_name: "User Example" },
    passwordResetEnabled: true,
    completeAuth: vi.fn(),
    clearAuth: vi.fn(),
    logout: vi.fn(),
  };
  mocks.uiPreferencesState = {
    locale: "en",
    theme: "system",
    setLocale: vi.fn(),
    setTheme: vi.fn(),
  };

  mocks.addWorkspaceSourceUrlMock.mockReset();
  mocks.completeWorkspaceDocumentUploadMock.mockReset();
  mocks.createWorkspaceNoteMock.mockReset();
  mocks.createWorkspaceDocumentUploadMock.mockReset();
  mocks.deleteWorkspaceDocumentMock.mockReset();
  mocks.deleteWorkspaceNoteMock.mockReset();
  mocks.getWorkspaceSourceContentMock.mockReset();
  mocks.getWorkspaceSourceParsedPreviewMock.mockReset();
  mocks.listWorkspaceNotesMock.mockReset();
  mocks.listWorkspaceSourcesMock.mockReset();
  mocks.lookupWorkspaceCitationMock.mockReset();
  mocks.promoteWorkspaceNoteMock.mockReset();
  mocks.reindexWorkspaceDocumentMock.mockReset();
  mocks.uploadWorkspaceDocumentFileMock.mockReset();
  mocks.updateWorkspaceNoteMock.mockReset();
  mocks.getWorkspaceSourceContentMock.mockResolvedValue({
    content: "Fallback content",
    summary: null,
  });
  mocks.getWorkspaceSourceParsedPreviewMock.mockResolvedValue({
    items: [],
    has_more: false,
    next_cursor: 0,
    summary: null,
  });
  mocks.lookupWorkspaceCitationMock.mockResolvedValue({
    doc_name: "Doc",
    content: "Matched excerpt",
    doc_id: "src-1",
    chunk_id: "chunk-1",
    page: 2,
    chunk_type: "text",
    asset_id: null,
    caption: null,
    image_url: null,
  });
  window.localStorage.clear();
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceRightRail", () => {
  it("uses web sources takeover without querying source and note panes", async () => {
    const user = userEvent.setup();
    const onCloseWebSources = vi.fn();

    renderHarness({
      activeWebSources: {
        sources: [
          {
            title: "Primary result",
            url: "https://example.test/primary",
            snippet: "Primary snippet",
          },
          {
            title: "Secondary result",
            url: "https://example.test/secondary",
            snippet: "Secondary snippet",
          },
        ],
      },
      onCloseWebSources,
    });

    expect(await screen.findByText("2 sources")).toBeTruthy();
    const primaryLink = screen.getByRole("link", { name: "Primary result" });
    expect(primaryLink.getAttribute("href")).toBe("https://example.test/primary");
    expect(screen.getByText("Primary snippet")).toBeTruthy();
    expect(screen.queryByRole("list", { name: "Sources list" })).toBeNull();
    expect(screen.queryByRole("list", { name: "Notes list" })).toBeNull();
    expect(mocks.listWorkspaceSourcesMock).not.toHaveBeenCalled();
    expect(mocks.listWorkspaceNotesMock).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Close sources" }));
    expect(onCloseWebSources).toHaveBeenCalledTimes(1);
  });

  it("auto-selects eligible sources, polls until terminal, and follows the focused source", async () => {
    vi.useFakeTimers();

    mocks.listWorkspaceSourcesMock
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-1", file_name: "beta.pdf", status: "processing", title: "Beta" }),
          buildSource({ id: "src-2", file_name: "alpha.pdf", status: "ready", title: "Alpha" }),
          buildSource({ id: "src-3", file_name: "gamma.pdf", status: "completed", title: "Gamma" }),
        ],
      })
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-1", file_name: "beta.pdf", status: "completed", title: "Beta" }),
          buildSource({ id: "src-2", file_name: "alpha.pdf", status: "ready", title: "Alpha" }),
          buildSource({ id: "src-3", file_name: "gamma.pdf", status: "completed", title: "Gamma" }),
        ],
      });
    mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });

    const { rerender } = renderHarness({ focusedSourceId: "src-2" });

    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
      await Promise.resolve();
    });

    const sourceList = screen.getByRole("list", { name: "Sources list" });
    const items = within(sourceList).getAllByRole("listitem");
    expect(items).toHaveLength(3);

    const alphaItem = screen.getByText("alpha.pdf").closest("li");
    expect(alphaItem?.className).toContain("listItemFocused");

    rerender(
      <QueryProvider>
        <Harness focusedSourceId="src-1" />
      </QueryProvider>,
    );
    const betaItem = screen.getByText("beta.pdf").closest("li");
    expect(betaItem?.className).toContain("listItemFocused");

    expect(mocks.listWorkspaceSourcesMock.mock.calls.length).toBeGreaterThanOrEqual(2);
    const refreshedItems = within(screen.getByRole("list", { name: "Sources list" })).getAllByRole("listitem");
    expect(within(refreshedItems[0]!).getByRole("button", { pressed: true })).toBeTruthy();
    expect(within(refreshedItems[1]!).getByRole("button", { pressed: true })).toBeTruthy();
    expect(within(refreshedItems[2]!).getByRole("button", { pressed: true })).toBeTruthy();

    const callsAfterPolling = mocks.listWorkspaceSourcesMock.mock.calls.length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
      await Promise.resolve();
    });

    expect(mocks.listWorkspaceSourcesMock.mock.calls.length).toBe(callsAfterPolling);
  });

  it("supports select all, add URL source, delete, and reindex actions", async () => {
    const user = userEvent.setup();

    mocks.listWorkspaceSourcesMock
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-1", file_name: "alpha.pdf", status: "processing", title: "Alpha" }),
          buildSource({ id: "src-2", file_name: "beta.pdf", status: "processing", title: "Beta" }),
        ],
      })
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-1", file_name: "alpha.pdf", status: "processing", title: "Alpha" }),
          buildSource({ id: "src-2", file_name: "beta.pdf", status: "processing", title: "Beta" }),
          buildSource({ id: "src-3", file_name: "gamma.pdf", status: "processing", title: "Gamma" }),
          buildSource({ id: "src-4", file_name: "doc.html", status: "processing", title: "Doc" }),
          buildSource({ id: "src-5", file_name: "news.html", status: "processing", title: "News" }),
        ],
      })
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-2", file_name: "beta.pdf", status: "processing", title: "Beta" }),
          buildSource({ id: "src-3", file_name: "gamma.pdf", status: "processing", title: "Gamma" }),
          buildSource({ id: "src-4", file_name: "doc.html", status: "processing", title: "Doc" }),
          buildSource({ id: "src-5", file_name: "news.html", status: "processing", title: "News" }),
        ],
      })
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-2", file_name: "beta.pdf", status: "processing", title: "Beta" }),
          buildSource({ id: "src-3", file_name: "gamma.pdf", status: "completed", title: "Gamma" }),
          buildSource({ id: "src-4", file_name: "doc.html", status: "processing", title: "Doc" }),
          buildSource({ id: "src-5", file_name: "news.html", status: "processing", title: "News" }),
        ],
      });
    mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });
    mocks.addWorkspaceSourceUrlMock
      .mockResolvedValueOnce({
        document_id: "src-4",
        upload_url: "https://upload.example.test/src-4",
        status: "processing",
      })
      .mockResolvedValueOnce({
        document_id: "src-5",
        upload_url: "https://upload.example.test/src-5",
        status: "processing",
      });
    mocks.deleteWorkspaceDocumentMock.mockResolvedValue(undefined);
    mocks.reindexWorkspaceDocumentMock.mockResolvedValue(undefined);

    renderHarness();

    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole("button", { name: "Select all" }));
    await waitFor(() => {
      expect(
        within(screen.getByRole("list", { name: "Sources list" }))
          .getAllByRole("listitem")
          .every((item) => within(item).getByRole("button", { pressed: true })),
      ).toBe(true);
    });

    await user.click(screen.getByRole("button", { name: "New Source" }));
    await user.click(screen.getByRole("tab", { name: "Web Link" }));
    await user.type(screen.getByLabelText("Source URLs"), "https://example.test/doc{enter}https://example.test/news");
    await user.click(screen.getByRole("button", { name: "Add Link" }));
    await waitFor(() => {
      expect(mocks.addWorkspaceSourceUrlMock).toHaveBeenNthCalledWith(1, "token-123", "ws-1", "https://example.test/doc");
    });
    expect(mocks.addWorkspaceSourceUrlMock).toHaveBeenNthCalledWith(2, "token-123", "ws-1", "https://example.test/news");

    expect(within(screen.getByRole("list", { name: "Sources list" })).getAllByRole("listitem")).toHaveLength(5);

    const alphaItem = screen.getByText("alpha.pdf").closest("li");
    expect(alphaItem).not.toBeNull();
    await user.click(within(alphaItem!).getByRole("button", { name: "Source actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    await waitFor(() => {
      expect(mocks.deleteWorkspaceDocumentMock).toHaveBeenCalledWith("token-123", "src-1");
    });

    const gammaItem = screen.getByText("gamma.pdf").closest("li");
    expect(gammaItem).not.toBeNull();
    await user.click(within(gammaItem!).getByRole("button", { name: "Source actions" }));
    await user.click(screen.getByRole("menuitem", { name: "Reindex" }));
    await waitFor(() => {
      expect(mocks.reindexWorkspaceDocumentMock).toHaveBeenCalledWith("token-123", "src-3");
    });
    expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(4);
  });

  it("opens the new source dialog on upload by default and uploads selected files from browse", async () => {
    const user = userEvent.setup();

    mocks.listWorkspaceSourcesMock
      .mockResolvedValueOnce({
        sources: [buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" })],
      })
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" }),
          buildSource({ id: "src-2", file_name: "notes.md", status: "processing", title: "Notes" }),
        ],
      });
    mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });
    mocks.createWorkspaceDocumentUploadMock.mockResolvedValue({
      document_id: "src-2",
      upload_url: "https://upload.example.test/src-2",
      status: "pending",
    });
    mocks.uploadWorkspaceDocumentFileMock.mockResolvedValue(undefined);
    mocks.completeWorkspaceDocumentUploadMock.mockResolvedValue(undefined);

    const { container } = renderHarness();

    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole("button", { name: "New Source" }));

    expect(screen.getByRole("tab", { name: "Upload File" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.queryByLabelText("Source URLs")).toBeNull();
    expect((screen.getByRole("button", { name: "Browse Files" }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByText(/Supported upload formats:/)).toBeTruthy();

    const fileInput = container.querySelector('input[type="file"]') as HTMLInputElement | null;
    expect(fileInput).not.toBeNull();

    const file = new File(["hello world"], "notes.md", { type: "text/markdown" });
    await user.upload(fileInput!, file);

    await waitFor(() => {
      expect(mocks.createWorkspaceDocumentUploadMock).toHaveBeenCalledWith("token-123", "ws-1", {
        filename: "notes.md",
        file_size: 11,
        mime_type: "text/markdown",
      });
    });
    expect(mocks.uploadWorkspaceDocumentFileMock).toHaveBeenCalledWith("https://upload.example.test/src-2", file);
    expect(mocks.completeWorkspaceDocumentUploadMock).toHaveBeenCalledWith("token-123", "src-2");
    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(2);
    });
    expect(within(screen.getByText("notes.md").closest("li")!).getByRole("button", { pressed: true })).toBeTruthy();
    expect(getWorkspaceUiState("ws-1").chatMode).toBe("rag");
    expect(getWorkspaceUiState("ws-1").chatModePreference).toBe("auto");
    expect(screen.queryByRole("dialog", { name: "Add New Source" })).toBeNull();
  });

  it("keeps chat mode off rag for queued uploads without ready sources", async () => {
    const user = userEvent.setup();

    mocks.listWorkspaceSourcesMock
      .mockResolvedValueOnce({ sources: [] })
      .mockResolvedValueOnce({
        sources: [buildSource({ id: "src-2", file_name: "notes.md", status: "queued", title: "Notes" })],
      });
    mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });
    mocks.createWorkspaceDocumentUploadMock.mockResolvedValue({
      document_id: "src-2",
      upload_url: "https://upload.example.test/src-2",
      status: "queued",
    });
    mocks.uploadWorkspaceDocumentFileMock.mockResolvedValue(undefined);
    mocks.completeWorkspaceDocumentUploadMock.mockResolvedValue(undefined);

    const { container } = renderHarness();

    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole("button", { name: "New Source" }));

    const fileInput = container.querySelector('input[type="file"]') as HTMLInputElement | null;
    expect(fileInput).not.toBeNull();

    const file = new File(["hello world"], "notes.md", { type: "text/markdown" });
    await user.upload(fileInput!, file);

    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(2);
    });

    expect(within(screen.getByText("notes.md").closest("li")!).getByRole("button", { pressed: true })).toBeTruthy();
    expect(getWorkspaceUiState("ws-1").chatMode).toBe("general");
    expect(getWorkspaceUiState("ws-1").chatModePreference).toBe("manual");
  });

  it("imports pasted text without a title field", async () => {
    const user = userEvent.setup();

    mocks.listWorkspaceSourcesMock
      .mockResolvedValueOnce({
        sources: [buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" })],
      })
      .mockResolvedValueOnce({
        sources: [
          buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" }),
          buildSource({ id: "src-2", file_name: "pasted-source.txt", status: "processing", title: "Pasted source" }),
        ],
      });
    mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });
    mocks.createWorkspaceDocumentUploadMock.mockResolvedValue({
      document_id: "src-2",
      upload_url: "https://upload.example.test/src-2",
      status: "pending",
    });
    mocks.uploadWorkspaceDocumentFileMock.mockResolvedValue(undefined);
    mocks.completeWorkspaceDocumentUploadMock.mockResolvedValue(undefined);

    renderHarness();

    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole("button", { name: "New Source" }));
    await user.click(screen.getByRole("tab", { name: "Paste Text" }));

    expect(screen.queryByLabelText("Title")).toBeNull();

    const saveButton = screen.getByRole("button", { name: "Save as Source" }) as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);

    await user.type(screen.getByLabelText("Text"), "Line 1{enter}Line 2");
    expect(saveButton.disabled).toBe(false);

    await user.click(saveButton);

    await waitFor(() => {
      expect(mocks.createWorkspaceDocumentUploadMock).toHaveBeenCalledWith("token-123", "ws-1", {
        filename: "pasted-source.txt",
        file_size: 13,
        mime_type: "text/plain",
      });
    });
    expect(mocks.completeWorkspaceDocumentUploadMock).toHaveBeenCalledWith("token-123", "src-2");
    expect(mocks.uploadWorkspaceDocumentFileMock).toHaveBeenCalledTimes(1);

    const uploadedFile = mocks.uploadWorkspaceDocumentFileMock.mock.calls[0]?.[1] as File;
    expect(uploadedFile.name).toBe("pasted-source.txt");
    expect(uploadedFile.type).toBe("text/plain");
    expect(uploadedFile.size).toBe(13);

    await waitFor(() => {
      expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(2);
    });
    expect(screen.queryByRole("dialog", { name: "Add New Source" })).toBeNull();
  });

  it("uses contextual source viewer and note editor takeover while keeping source/note linkage stable", async () => {
    mocks.listWorkspaceSourcesMock.mockResolvedValue({
      sources: [buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" })],
    });
    mocks.listWorkspaceNotesMock.mockResolvedValue({
      notes: [buildNote({ id: "note-1", title: "Alpha note", content: "Body", preview: "Body" })],
    });
    mocks.getWorkspaceSourceParsedPreviewMock.mockResolvedValue({
      items: [
        {
          kind: "text",
          text: "Matched excerpt with more context",
          page: 2,
          cursor: 40,
        },
      ],
      has_more: false,
      next_cursor: 0,
      summary: "Summary for Alpha",
    });
    mocks.updateWorkspaceNoteMock.mockResolvedValue({
      note: buildNote({
        id: "note-1",
        title: "Alpha note",
        content: "Body updated",
        preview: "Body updated",
      }),
    });

    renderHarness({ focusedSourceId: "src-1" });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "alpha.pdf" })).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: "alpha.pdf" }));

    await waitFor(() => {
      expect(screen.getByRole("region", { name: "Source viewer" })).toBeTruthy();
      expect(mocks.getWorkspaceSourceParsedPreviewMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.getByText("Summary for Alpha")).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: /Alpha note Body/i }));
    expect(screen.queryByRole("region", { name: "Source viewer" })).toBeNull();
    const noteContentEditor = screen.getByLabelText("Content");
    expect(screen.getByRole("heading", { name: "Body" })).toBeTruthy();
    noteContentEditor.textContent = "Body updated";
    fireEvent.input(noteContentEditor);
    expect(screen.getByRole("button", { name: "Save note" })).toBeTruthy();

    await waitFor(() => {
      expect(mocks.updateWorkspaceNoteMock).toHaveBeenCalledWith("token-123", "ws-1", "note-1", {
        title: "Body updated",
        content: "Body updated",
      });
    });

    expect(screen.queryByText("Syncing notes...")).toBeNull();
    expect(screen.queryByText("Notes synced.")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => {
      expect(screen.getByRole("region", { name: "Source viewer" })).toBeTruthy();
    });
    expect(screen.getByText("Matched excerpt with more context")).toBeTruthy();
  });

  it("creates, promotes, and deletes notes", async () => {
    vi.useFakeTimers();

    mocks.listWorkspaceSourcesMock
      .mockResolvedValueOnce({ sources: [buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready" })] })
      .mockResolvedValueOnce({ sources: [buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready" })] });
    mocks.listWorkspaceNotesMock
      .mockResolvedValueOnce({
        notes: [
          buildNote({ id: "note-1", title: "Alpha", content: "Alpha body", preview: "Alpha body" }),
          buildNote({
            id: "note-2",
            title: "Beta",
            content: "Beta body",
            preview: "Beta body",
            updated_at: "2026-04-19T00:00:00Z",
          }),
        ],
      })
      .mockResolvedValueOnce({
        notes: [
          buildNote({ id: "note-1", title: "Alpha", content: "Alpha body", preview: "Alpha body" }),
          buildNote({
            id: "note-2",
            title: "Beta",
            content: "Beta body",
            preview: "Beta body",
            updated_at: "2026-04-19T00:00:00Z",
          }),
          buildNote({ id: "note-3", title: "", content: "", preview: "" }),
        ],
      })
      .mockResolvedValue({
        notes: [
          buildNote({ id: "note-1", title: "Alpha", content: "Alpha body", preview: "Alpha body" }),
          buildNote({
            id: "note-2",
            title: "Beta",
            content: "Beta body",
            preview: "Beta body",
            updated_at: "2026-04-19T00:00:00Z",
          }),
          buildNote({ id: "note-3", title: "", content: "", preview: "" }),
        ],
      });
    mocks.createWorkspaceNoteMock.mockResolvedValueOnce({
      note: buildNote({ id: "note-3", title: "", content: "", preview: "" }),
    });
    mocks.promoteWorkspaceNoteMock.mockResolvedValueOnce({
      note: buildNote({
        id: "note-3",
        title: "",
        content: "",
        preview: "",
        promoted_document_id: "src-3",
        promoted_at: "2026-04-20T00:00:00Z",
      }),
      source_id: "src-3",
    });
    mocks.deleteWorkspaceNoteMock.mockResolvedValueOnce(undefined);

    renderHarness();

    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.listWorkspaceNotesMock).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "New note" }));
    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.createWorkspaceNoteMock).toHaveBeenCalledWith("token-123", "ws-1", {
      title: null,
      content: null,
    });

    expect(screen.getByRole("button", { name: "Save note" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Untitled note" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Convert to source" }));
    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.promoteWorkspaceNoteMock).toHaveBeenCalledWith("token-123", "ws-1", "note-3");
    expect(mocks.listWorkspaceSourcesMock).toHaveBeenCalledTimes(2);

    const untitledNoteItem = screen.getByText("Untitled note").closest("li");
    expect(untitledNoteItem).not.toBeNull();
    fireEvent.click(within(untitledNoteItem!).getByRole("button", { name: "Note actions for Untitled note" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete note" }));
    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.deleteWorkspaceNoteMock).toHaveBeenCalledWith("token-123", "ws-1", "note-3");
    expect(screen.getByRole("button", { name: "New note" })).toBeTruthy();
  });

  it("reuses cached source preview queries when reopening the same source", async () => {
    mocks.listWorkspaceSourcesMock.mockResolvedValue({
      sources: [buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" })],
    });
    mocks.listWorkspaceNotesMock.mockResolvedValue({ notes: [] });
    mocks.getWorkspaceSourceParsedPreviewMock.mockResolvedValue({
      items: [
        {
          kind: "text",
          text: "Matched excerpt with more context",
          page: 2,
          cursor: 40,
        },
      ],
      has_more: false,
      next_cursor: 0,
      summary: "Summary for Alpha",
    });

    render(
      <QueryProvider>
        <Harness />
      </QueryProvider>,
    );

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "alpha.pdf" })).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: "alpha.pdf" }));

    await waitFor(() => {
      expect(screen.getByText("Matched excerpt with more context")).toBeTruthy();
    });

    expect(mocks.getWorkspaceSourceParsedPreviewMock).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Close" }));

    fireEvent.click(screen.getByRole("button", { name: "alpha.pdf" }));

    await waitFor(() => {
      expect(screen.getByText("Matched excerpt with more context")).toBeTruthy();
    });

    expect(mocks.getWorkspaceSourceParsedPreviewMock).toHaveBeenCalledTimes(1);
  });

  it("renders right rail controls from central zh-CN messages", async () => {
    mocks.uiPreferencesState.locale = "zh-CN";
    mocks.listWorkspaceSourcesMock.mockResolvedValue({
      sources: [
        buildSource({ id: "src-1", file_name: "alpha.pdf", status: "ready", title: "Alpha" }),
        buildSource({ id: "src-2", file_name: "notes.md", status: "queued", title: "Notes" }),
      ],
    });
    mocks.listWorkspaceNotesMock.mockResolvedValue({
      notes: [buildNote({ id: "note-1", title: "", content: "", preview: "" })],
    });

    renderHarness();

    await waitFor(() => {
      expect(screen.getByRole("list", { name: "资料列表" })).toBeTruthy();
    });

    expect(screen.getByRole("button", { name: "全选" })).toBeTruthy();
    expect(screen.getByRole("list", { name: "资料列表" })).toBeTruthy();
    expect(screen.getByRole("list", { name: "笔记列表" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "新建笔记" })).toBeTruthy();
    expect(screen.getByText("未命名笔记")).toBeTruthy();
    expect(screen.getByText("还没有内容。")).toBeTruthy();
    expect(screen.getByTitle("排队中")).toBeTruthy();
  });
});
