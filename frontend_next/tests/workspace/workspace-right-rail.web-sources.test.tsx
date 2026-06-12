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

describe("WorkspaceRightRail web sources", () => {
  it("uses web sources takeover without querying source and note panes", async () => {
    const user = userEvent.setup();
    const onCloseWebSources = vi.fn();

    renderRightRailHarness({
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
});
