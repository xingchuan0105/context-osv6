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

describe("WorkspaceRightRail sources", () => {
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

    const { rerender } = renderRightRailHarness({ focusedSourceId: "src-2" });

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
        <RightRailHarness focusedSourceId="src-1" />
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

    renderRightRailHarness();

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
});
