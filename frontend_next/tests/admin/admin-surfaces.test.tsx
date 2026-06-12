import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const originalAnchorClick = HTMLAnchorElement.prototype.click;

vi.mock("next/navigation", () => ({
  useParams: () => ({
    org_id: "org-1",
  }),
}));

vi.mock("../../lib/auth/context", () => ({
  useAuth: () => mocks.authState,
}));

vi.mock("../../lib/ui-preferences", () => ({
  useUiPreferences: () => mocks.uiPreferencesState,
}));

vi.mock("../../lib/admin/client", () => ({
  listAdminOrganizations: mocks.listAdminOrganizationsMock,
  getAdminOrganization: mocks.getAdminOrganizationMock,
  listAdminUsersForOrganization: mocks.listAdminUsersForOrganizationMock,
  getAdminUsageForOrganization: mocks.getAdminUsageForOrganizationMock,
  updateAdminOrganizationBlocked: mocks.updateAdminOrganizationBlockedMock,
  getAdminHealth: mocks.getAdminHealthMock,
  getAdminBillingOverview: mocks.getAdminBillingOverviewMock,
  getAdminRagHealth: mocks.getAdminRagHealthMock,
  listAdminFeatureFlags: mocks.listAdminFeatureFlagsMock,
  requestAdminFeatureFlagChange: mocks.requestAdminFeatureFlagChangeMock,
  reviewAdminFeatureFlagChange: mocks.reviewAdminFeatureFlagChangeMock,
  listAdminFeatureFlagChangeRequests: mocks.listAdminFeatureFlagChangeRequestsMock,
  getAdminWorkerStatus: mocks.getAdminWorkerStatusMock,
  getAdminDegradationStatus: mocks.getAdminDegradationStatusMock,
  listAdminAuditLogs: mocks.listAdminAuditLogsMock,
  exportAdminAuditLogsCsv: mocks.exportAdminAuditLogsCsvMock,
}));

import { AdminOrganizationsSurface, AdminUsageSurface } from "../../components/admin/admin-core-surfaces";
import { AdminAuditLogsSurface, AdminFeatureFlagsSurface } from "../../components/admin/admin-ops-surfaces";

const mocks = vi.hoisted(() => globalThis.__mockProviders.createAdminSurfacesMocks());



function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
}

function renderWithQueryClient(children: ReactNode, queryClient = createQueryClient()) {
  const rendered = render(<QueryClientProvider client={queryClient}>{children}</QueryClientProvider>);

  return {
    queryClient,
    ...rendered,
    rerenderWithClient(nextChildren: ReactNode) {
      rendered.rerender(<QueryClientProvider client={queryClient}>{nextChildren}</QueryClientProvider>);
    },
  };
}

describe("admin surfaces", () => {
  beforeEach(() => {
    window.localStorage.clear();
    mocks.authState = {
      token: "token-123",
      user: {
        id: "user-1",
        email: "owner@example.com",
        full_name: "Owner",
      },
    };
    mocks.uiPreferencesState = {
      locale: "en",
    };
    mocks.listAdminOrganizationsMock.mockReset();
    mocks.getAdminOrganizationMock.mockReset();
    mocks.listAdminUsersForOrganizationMock.mockReset();
    mocks.getAdminUsageForOrganizationMock.mockReset();
    mocks.updateAdminOrganizationBlockedMock.mockReset();
    mocks.getAdminHealthMock.mockReset();
    mocks.getAdminBillingOverviewMock.mockReset();
    mocks.getAdminRagHealthMock.mockReset();
    mocks.listAdminFeatureFlagsMock.mockReset();
    mocks.requestAdminFeatureFlagChangeMock.mockReset();
    mocks.reviewAdminFeatureFlagChangeMock.mockReset();
    mocks.listAdminFeatureFlagChangeRequestsMock.mockReset();
    mocks.getAdminWorkerStatusMock.mockReset();
    mocks.getAdminDegradationStatusMock.mockReset();
    mocks.listAdminAuditLogsMock.mockReset();
    mocks.exportAdminAuditLogsCsvMock.mockReset();

    mocks.listAdminOrganizationsMock.mockResolvedValue([
      {
        id: "org-1",
        name: "Alpha Org",
        plan: "pro",
        user_count: 12,
        notebook_count: 5,
        query_count: 120,
        blocked: false,
        created_at: 1_713_600_000,
      },
      {
        id: "org-2",
        name: "Beta Org",
        plan: "team",
        user_count: 4,
        notebook_count: 2,
        query_count: 80,
        blocked: true,
        created_at: 1_713_600_100,
      },
    ]);
    mocks.getAdminUsageForOrganizationMock.mockImplementation(async (_token: string, orgId: string, period: string) => {
      if (period === "7d") {
        return orgId === "org-1"
          ? { total_requests: 70, total_tokens: 700, total_documents: 7 }
          : { total_requests: 30, total_tokens: 300, total_documents: 3 };
      }

      return orgId === "org-1"
        ? { total_requests: 120, total_tokens: 1200, total_documents: 12 }
        : { total_requests: 80, total_tokens: 1800, total_documents: 8 };
    });
    mocks.updateAdminOrganizationBlockedMock.mockResolvedValue(undefined);
    mocks.listAdminFeatureFlagsMock.mockResolvedValue([
      {
        key: "guard_output",
        category: "guard",
        description: "Block unsafe output",
        enabled: false,
        effective_enabled: false,
        config_ready: true,
        requires_config: false,
        source: "config",
        updated_at: 1_713_600_000,
        has_pending_request: false,
      },
    ]);
    mocks.listAdminFeatureFlagChangeRequestsMock.mockResolvedValue([
      {
        id: "req-1",
        flag_key: "guard_output",
        current_enabled: false,
        requested_enabled: true,
        reason: "Enable beta",
        status: "pending",
        requested_by: "owner@example.com",
        reviewed_by: null,
        review_note: null,
        created_at: 1_713_600_100,
        reviewed_at: null,
        executed_at: null,
      },
    ]);
    mocks.requestAdminFeatureFlagChangeMock.mockResolvedValue({
      id: "req-2",
      flag_key: "guard_output",
      current_enabled: false,
      requested_enabled: true,
      reason: "Rollout beta",
      status: "pending",
      requested_by: "owner@example.com",
      reviewed_by: null,
      review_note: null,
      created_at: 1_713_600_200,
      reviewed_at: null,
      executed_at: null,
    });
    mocks.reviewAdminFeatureFlagChangeMock.mockResolvedValue({
      id: "req-1",
      flag_key: "guard_output",
      current_enabled: false,
      requested_enabled: true,
      reason: "Enable beta",
      status: "approved",
      requested_by: "owner@example.com",
      reviewed_by: "reviewer@example.com",
      review_note: "Looks good",
      created_at: 1_713_600_100,
      reviewed_at: 1_713_600_300,
      executed_at: 1_713_600_301,
    });
    mocks.listAdminAuditLogsMock.mockResolvedValue({
      items: [
        {
          id: 1,
          actor_id: "user-1",
          action: "task_failed",
          resource_type: "document",
          resource_id: "doc-1",
          org_id: "org-1",
          created_at: 1_713_600_400,
        },
      ],
      total: 1,
      page: 1,
      per_page: 25,
    });
    mocks.exportAdminAuditLogsCsvMock.mockResolvedValue("id,action\n1,task_failed\n");

    vi.stubGlobal(
      "URL",
      Object.assign(globalThis.URL, {
        createObjectURL: vi.fn(() => "blob:test"),
        revokeObjectURL: vi.fn(),
      }),
    );
    HTMLAnchorElement.prototype.click = vi.fn();
  });

  afterEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
    HTMLAnchorElement.prototype.click = originalAnchorClick;
  });

  it("loads organizations and toggles block state", async () => {
    const user = userEvent.setup();

    renderWithQueryClient(<AdminOrganizationsSurface />);

    expect(await screen.findByText("Alpha Org")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Block" }));

    await waitFor(() => {
      expect(mocks.updateAdminOrganizationBlockedMock).toHaveBeenCalledWith("token-123", "org-1", true);
    });

    const alphaRow = screen.getByRole("link", { name: "Alpha Org" }).closest("tr");
    expect(alphaRow).toBeTruthy();
    expect(within(alphaRow as HTMLTableRowElement).getByRole("button", { name: "Unblock" })).toBeTruthy();
  });

  it("scopes organization cache to the signed-in actor inside the same query client", async () => {
    let resolveSecondActorOrganizations: (value: Awaited<ReturnType<typeof mocks.listAdminOrganizationsMock>>) => void =
      () => {
        throw new Error("Expected the second actor organizations request to be pending.");
      };

    mocks.listAdminOrganizationsMock.mockImplementation((token: string) => {
      if (token === "token-a") {
        return Promise.resolve([
          {
            id: "org-a",
            name: "Alpha Org",
            plan: "pro",
            user_count: 12,
            notebook_count: 5,
            query_count: 120,
            blocked: false,
            created_at: 1_713_600_000,
          },
        ]);
      }

      if (token === "token-b") {
        return new Promise((resolve) => {
          resolveSecondActorOrganizations = resolve;
        });
      }

      return Promise.resolve([]);
    });

    mocks.authState = {
      token: "token-a",
      user: {
        id: "user-a",
        email: "alpha@example.com",
        full_name: "Alpha",
      },
    };

    const view = renderWithQueryClient(<AdminOrganizationsSurface />);

    expect(await screen.findByText("Alpha Org")).toBeTruthy();

    mocks.authState = {
      token: "token-b",
      user: {
        id: "user-b",
        email: "beta@example.com",
        full_name: "Beta",
      },
    };

    view.rerenderWithClient(<AdminOrganizationsSurface />);

    await waitFor(() => {
      expect(mocks.listAdminOrganizationsMock).toHaveBeenCalledWith("token-b");
    });

    await waitFor(() => {
      expect(screen.queryByText("Alpha Org")).toBeNull();
    });

    expect(screen.getByText("Loading organizations...")).toBeTruthy();

    resolveSecondActorOrganizations([
      {
        id: "org-b",
        name: "Beta Org",
        plan: "team",
        user_count: 6,
        notebook_count: 3,
        query_count: 48,
        blocked: false,
        created_at: 1_713_600_200,
      },
    ]);

    expect(await screen.findByText("Beta Org")).toBeTruthy();
    expect(screen.queryByText("Alpha Org")).toBeNull();
  });

  it("aggregates usage across organizations and reloads when the period changes", async () => {
    const user = userEvent.setup();

    renderWithQueryClient(<AdminUsageSurface />);

    await waitFor(() => {
      expect(mocks.getAdminUsageForOrganizationMock).toHaveBeenCalledWith("token-123", "org-1", "30d");
      expect(mocks.getAdminUsageForOrganizationMock).toHaveBeenCalledWith("token-123", "org-2", "30d");
    });

    expect(await screen.findByText("Platform statistics")).toBeTruthy();
    expect(screen.getAllByText("200").length).toBeGreaterThan(0);
    expect(screen.getAllByText("3.0K").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "7d" }));

    await waitFor(() => {
      expect(mocks.getAdminUsageForOrganizationMock).toHaveBeenCalledWith("token-123", "org-1", "7d");
      expect(mocks.getAdminUsageForOrganizationMock).toHaveBeenCalledWith("token-123", "org-2", "7d");
    });
  });

  it("submits and reviews feature-flag change requests", async () => {
    const user = userEvent.setup();

    renderWithQueryClient(<AdminFeatureFlagsSurface />);

    expect((await screen.findAllByText("guard_output")).length).toBeGreaterThan(0);

    await user.type(screen.getByPlaceholderText("Reason for this change request"), "Rollout beta");
    await user.click(screen.getByRole("button", { name: "Request enable" }));

    await waitFor(() => {
      expect(mocks.requestAdminFeatureFlagChangeMock).toHaveBeenCalledWith("token-123", "guard_output", true, "Rollout beta");
    });

    await user.type(screen.getByPlaceholderText("Optional review note"), "Looks good");
    await user.click(screen.getByRole("button", { name: "Approve & execute" }));

    await waitFor(() => {
      expect(mocks.reviewAdminFeatureFlagChangeMock).toHaveBeenCalledWith("token-123", "req-1", true, "Looks good");
    });
  });

  it("loads audit logs with filters and exports csv", async () => {
    const user = userEvent.setup();

    renderWithQueryClient(<AdminAuditLogsSurface />);

    expect(await screen.findByText("Task failed")).toBeTruthy();

    mocks.listAdminAuditLogsMock.mockClear();
    await user.type(screen.getByLabelText("Action"), "task_failed");

    await waitFor(() => {
      expect(mocks.listAdminAuditLogsMock).toHaveBeenLastCalledWith("token-123", {
        query: "",
        action: "task_failed",
        resource_type: null,
        actor: null,
        window: null,
        page: 1,
        per_page: 25,
      });
    });

    await user.click(screen.getByRole("button", { name: "Export CSV" }));

    await waitFor(() => {
      expect(mocks.exportAdminAuditLogsCsvMock).toHaveBeenCalledWith("token-123", {
        query: "",
        action: "task_failed",
        resource_type: null,
        actor: null,
        window: null,
      });
    });
  });
});
