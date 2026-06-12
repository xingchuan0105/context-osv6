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

describe("WorkspaceRightRail viewer", () => {
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

    renderRightRailHarness({ focusedSourceId: "src-1" });

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
        <RightRailHarness />
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
});
