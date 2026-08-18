import type { ReactElement } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../lib/runtime/tauri-ipc", () => ({
  isTauri: vi.fn(),
}));

vi.mock("../../lib/desktop/tauri-publish", () => ({
  getPublishStatus: vi.fn(),
  publishWorkspace: vi.fn(),
  listenPublishProgress: vi.fn(async () => () => {}),
  bindDesktopCloudShare: vi.fn(),
  cloudShareRequest: vi.fn(),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => ({ token: "local-token" }),
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => ({ locale: "zh-CN" as const }),
}));

vi.mock("../../lib/share/client", async () => {
  const actual = await vi.importActual<typeof import("../../lib/share/client")>(
    "../../lib/share/client",
  );
  return {
    ...actual,
    getShareSettings: vi.fn(),
    getShareQuota: vi.fn(),
    listMembers: vi.fn(),
    getShareAnalytics: vi.fn(),
    getShareAccessLogs: vi.fn(),
  };
});

vi.mock("../../lib/api-access/client", async () => {
  const actual = await vi.importActual("../../lib/api-access/client");
  return {
    ...actual,
    listApiKeys: vi.fn().mockResolvedValue({ api_keys: [] }),
  };
});

import { WorkspaceShareQuickModal } from "../../components/share/workspace-share-quick-modal";
import {
  getPublishStatus,
  publishWorkspace,
} from "../../lib/desktop/tauri-publish";
import { isTauri } from "../../lib/runtime/tauri-ipc";
import {
  getShareAnalytics,
  getShareAccessLogs,
  getShareQuota,
  getShareSettings,
  listMembers,
} from "../../lib/share/client";

function renderWithQuery(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

describe("desktop share publish gate", () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReset();
    vi.mocked(getPublishStatus).mockReset();
    vi.mocked(publishWorkspace).mockReset();
    vi.mocked(getShareSettings).mockReset();
    vi.mocked(getShareQuota).mockReset();
    vi.mocked(listMembers).mockReset();
    vi.mocked(getShareQuota).mockResolvedValue({ used: 0, max: 3, plan_id: "free" });
    vi.mocked(listMembers).mockResolvedValue({ members: [] });
    vi.mocked(getShareAnalytics).mockResolvedValue({
      total_views: 0,
      total_unique_visitors: 0,
      views_by_day: {},
    });
    vi.mocked(getShareAccessLogs).mockResolvedValue({ logs: [] });
  });

  it("shows publish CTA before the cloud replica is ready", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getPublishStatus).mockResolvedValue({ status: "never" });

    renderWithQuery(
      <WorkspaceShareQuickModal open workspaceId="local-ws-1" onClose={() => undefined} />,
    );

    expect(await screen.findByTestId("desktop-publish-cta")).toBeInTheDocument();
    expect(screen.getByTestId("share-switch")).toBeDisabled();
    expect(getShareSettings).not.toHaveBeenCalled();
  });

  it("shows the share switch after publish is ready", async () => {
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(getPublishStatus).mockResolvedValue({
      status: "ready",
      cloud_workspace_id: "cloud-ws-9",
    });
    vi.mocked(getShareSettings).mockResolvedValue({
      share_token: "",
      access_level: "private",
      expires_at: null,
      allow_download: false,
      anon_question_limit: 10,
      member_question_limit: null,
    });

    renderWithQuery(
      <WorkspaceShareQuickModal open workspaceId="local-ws-1" onClose={() => undefined} />,
    );

    await waitFor(() => {
      expect(screen.queryByTestId("desktop-publish-cta")).not.toBeInTheDocument();
      expect(screen.getByTestId("share-switch")).not.toBeDisabled();
    });
    await waitFor(() => {
      expect(getShareSettings).toHaveBeenCalledWith("local-token", "cloud-ws-9");
    });
  });
});
