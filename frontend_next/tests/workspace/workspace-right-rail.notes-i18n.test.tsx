import { fireEvent, render, screen, within, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createWorkspaceRightRailMocks());

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

import { resetWorkspaceRightRailMocks } from "./helpers/workspace-right-rail.setup";
import {
  RightRailHarness,
  buildNote,
  buildSource,
  getWorkspaceUiState,
  renderRightRailHarness,
} from "./helpers/workspace-right-rail.harness";
import { QueryProvider } from "../../lib/query/provider";

beforeEach(() => {
  resetWorkspaceRightRailMocks(mocks);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("WorkspaceRightRail notes and i18n", () => {
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

    renderRightRailHarness();

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

    renderRightRailHarness();

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
