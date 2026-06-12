import { fireEvent, screen, waitFor, within } from "@testing-library/react";
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

describe("WorkspaceSurface shell", () => {
  it("renames the workspace title from the top bar", async () => {
    const user = userEvent.setup();

    renderWorkspaceSurface("ws-1");

    await user.click(await screen.findByRole("button", { name: "工作区标题" }));
    await user.clear(screen.getByLabelText("工作区标题"));
    await user.type(screen.getByLabelText("工作区标题"), "Renamed Workspace{enter}");

    await waitFor(() => {
      expect(mocks.updateWorkspaceMock).toHaveBeenCalledWith("token-123", "ws-1", {
        name: "Renamed Workspace",
        description: "A workspace",
      });
    });
  });

  it("opens mobile history and right drawers from the stored toggle state", async () => {
    setMobileViewport(true);
    workspaceUiStore.getState().setHistoryRailOpen("ws-1", true);
    workspaceUiStore.getState().setRightRailOpen("ws-1", false);

    const firstRender = renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");
    expect(screen.getByTestId("mobile-history-drawer")).toBeTruthy();
    expect(screen.queryByTestId("mobile-right-drawer")).toBeNull();

    firstRender.unmount();

    workspaceUiStore.getState().setHistoryRailOpen("ws-1", false);
    workspaceUiStore.getState().setRightRailOpen("ws-1", true);

    renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");
    expect(screen.queryByTestId("mobile-history-drawer")).toBeNull();
    expect(screen.getByTestId("mobile-right-drawer")).toBeTruthy();
  });

  it("resizes desktop rails through the visible separators", async () => {
    renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");

    const [historyResizer] = screen.getAllByRole("separator");

    fireEvent.mouseDown(historyResizer, { clientX: 200 });
    fireEvent.mouseMove(window, { clientX: 180 });
    fireEvent.mouseUp(window);

    expect(workspaceUiStore.getState().workspaces["ws-1"]?.historyRailWidth).toBe(300);
  });

  it("supports pointer-based rail resizing for webview-style input", async () => {
    renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");

    const [, rightResizer] = screen.getAllByRole("separator");

    fireEvent.pointerDown(rightResizer, { clientX: 1200 });
    fireEvent.pointerMove(window, { clientX: 1120 });
    fireEvent.pointerUp(window);

    expect(workspaceUiStore.getState().workspaces["ws-1"]?.rightRailWidth).toBe(392);
  });

  it("supports touch-based rail resizing for embedded webviews", async () => {
    renderWorkspaceSurface("ws-1");

    await screen.findByLabelText("工作区标题");

    const [historyResizer] = screen.getAllByRole("separator");

    fireEvent.touchStart(historyResizer, {
      touches: [{ clientX: 200 }],
    });
    fireEvent.touchMove(window, {
      touches: [{ clientX: 260 }],
    });
    fireEvent.touchEnd(window);

    expect(workspaceUiStore.getState().workspaces["ws-1"]?.historyRailWidth).toBe(320);
  });

  it("renders warning toast when soft limit hit on 5h", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 85000, limit: 100000, percentage: 85, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });

    renderWorkspaceSurface("ws-1");
    await waitFor(() => expect(screen.getByText(/5h 用量已用 85%/)).toBeTruthy());
  });

  it("renders warning toast at 95% tier on 7d window", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 1000, limit: 100000, percentage: 1, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 390000, limit: 400000, percentage: 97, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: false, rolling_7d: true },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });

    renderWorkspaceSurface("ws-1");
    await waitFor(() => expect(screen.getByText(/7d 用量已用 97%/)).toBeTruthy());
  });

  it("redirects to paywall when hard limit hit", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 100000, limit: 100000, percentage: 100, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 200000, limit: 400000, percentage: 50, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: false },
      hard_limit_hit: { rolling_5h: true, rolling_7d: false },
    });

    renderWorkspaceSurface("ws-1");
    await waitFor(() => {
      expect(mocks.pushMock).toHaveBeenCalledWith("/upgrade/paywall?reason=5h");
    });
  });

  it("prefers higher-pressure window when both hard limits hit", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 100000, limit: 100000, percentage: 100, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 450000, limit: 400000, percentage: 100, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: true },
      hard_limit_hit: { rolling_5h: true, rolling_7d: true },
    });

    renderWorkspaceSurface("ws-1");
    await waitFor(() => {
      expect(mocks.pushMock).toHaveBeenCalledWith("/upgrade/paywall?reason=7d");
    });
  });

  it("prefers higher-pressure window when both soft limits hit", async () => {
    mocks.getUsageWindowMock.mockResolvedValue({
      plan_id: "free",
      rolling_5h: { used: 85000, limit: 100000, percentage: 85, reset_at: "2099-01-01T00:00:00Z" },
      rolling_7d: { used: 390000, limit: 400000, percentage: 97, reset_at: "2099-01-01T00:00:00Z" },
      soft_limit_hit: { rolling_5h: true, rolling_7d: true },
      hard_limit_hit: { rolling_5h: false, rolling_7d: false },
    });

    renderWorkspaceSurface("ws-1");
    await waitFor(() => expect(screen.getByText(/7d 用量已用 97%/)).toBeTruthy());
  });
});
