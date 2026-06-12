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

describe("WorkspaceRightRail upload", () => {
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

    const { container } = renderRightRailHarness();

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

    const { container } = renderRightRailHarness();

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
    expect(getWorkspaceUiState("ws-1").chatMode).toBe("chat");
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

    renderRightRailHarness();

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
});
