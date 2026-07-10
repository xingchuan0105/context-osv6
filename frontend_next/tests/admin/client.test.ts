import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  exportAdminAuditLogsCsv,
  getAdminBillingOverview,
  getAdminDegradationStatus,
  getAdminHealth,
  getAdminAccount,
  getAdminRagHealth,
  getAdminUsageForAccount,
  getAdminWorkerStatus,
  listAdminAuditLogs,
  listAdminFeatureFlagChangeRequests,
  listAdminFeatureFlags,
  listAdminAccounts,
  listAdminUsersForAccount,
  requestAdminFeatureFlagChange,
  reviewAdminFeatureFlagChange,
  updateAdminAccountBlocked,
} from "../../lib/admin/client";

const fetchMock = vi.fn();

beforeEach(() => {
  process.env.NEXT_PUBLIC_API_BASE_URL = "https://api.example.test";
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  delete process.env.NEXT_PUBLIC_API_BASE_URL;
  vi.unstubAllGlobals();
});

describe("admin client", () => {
  it("maps account, user, usage, and health admin endpoints", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: [
              {
                id: "owner-1",
                name: "Alpha Org",
                created_at: 1_713_600_000,
                blocked: false,
                user_count: 12,
                document_count: 34,
                query_count: 56,
              },
            ],
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              id: "owner-1",
              name: "Alpha Org",
              created_at: 1_713_600_000,
              blocked: true,
              user_count: 12,
              document_count: 34,
              query_count: 56,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: [
              {
                id: "user-1",
                email: "owner@example.com",
                owner_user_id: "owner-1",
                role: "owner",
                created_at: 1_713_600_100,
              },
            ],
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              owner_user_id: "owner-1",
              period: "30d",
              query_count: 99,
              document_count: 11,
              chunk_count: 2222,
              storage_bytes: 4096,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ ok: true, data: {}, error: null }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              status: "ok",
              version: "2026.04.20",
              uptime_secs: 3600,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );

    await expect(listAdminAccounts("token-123")).resolves.toEqual([
      {
        id: "owner-1",
        name: "Alpha Org",
        plan: "N/A",
        user_count: 12,
        workspace_count: 34,
        query_count: 56,
        blocked: false,
        created_at: 1_713_600_000,
      },
    ]);

    await expect(getAdminAccount("token-123", "owner-1")).resolves.toEqual({
      id: "owner-1",
      name: "Alpha Org",
      plan: "N/A",
      user_count: 12,
      workspace_count: 34,
      query_count: 56,
      blocked: true,
      created_at: 1_713_600_000,
    });

    await expect(listAdminUsersForAccount("token-123", "owner-1")).resolves.toEqual([
      {
        id: "user-1",
        email: "owner@example.com",
        full_name: "",
        owner_user_id: "owner-1",
        role: "owner",
        created_at: 1_713_600_100,
        last_active_at: null,
      },
    ]);

    await expect(getAdminUsageForAccount("token-123", "owner-1", "30d")).resolves.toEqual({
      total_requests: 99,
      total_tokens: 2222,
      total_documents: 11,
    });

    await expect(updateAdminAccountBlocked("token-123", "owner-1", true)).resolves.toBeUndefined();

    await expect(getAdminHealth("token-123")).resolves.toEqual({
      status: "ok",
      service: "avrag-api",
      version: "2026.04.20",
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/admin/accounts",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/admin/accounts/owner-1",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/v1/admin/users?owner_user_id=owner-1",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "https://api.example.test/api/v1/admin/usage?owner_user_id=owner-1&period=30d",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock.mock.calls[4]?.[1]).toEqual(
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          owner_user_id: "owner-1",
          blocked: true,
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      "https://api.example.test/api/v1/admin/health",
      expect.objectContaining({ method: "GET" }),
    );
  });

  it("uses billing, feature-flag, worker, degradation, and audit endpoints", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              active_subscriptions: 8,
              past_due_subscriptions: 1,
              unpaid_subscriptions: 2,
              canceled_subscriptions: 3,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              failed_documents: 4,
              queued_tasks: 5,
              processing_tasks: 6,
              recent_guard_events: 7,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: [
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
            ],
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: [
              {
                id: "req-1",
                flag_key: "guard_output",
                current_enabled: false,
                requested_enabled: true,
                reason: "Enable rollout",
                status: "pending",
                requested_by: "owner@example.com",
                reviewed_by: null,
                review_note: null,
                created_at: 1_713_600_100,
                reviewed_at: null,
                executed_at: null,
              },
            ],
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              id: "req-2",
              flag_key: "guard_output",
              current_enabled: false,
              requested_enabled: true,
              reason: "Enable rollout",
              status: "pending",
              requested_by: "owner@example.com",
              reviewed_by: null,
              review_note: null,
              created_at: 1_713_600_200,
              reviewed_at: null,
              executed_at: null,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              id: "req-1",
              flag_key: "guard_output",
              current_enabled: false,
              requested_enabled: true,
              reason: "Enable rollout",
              status: "approved",
              requested_by: "owner@example.com",
              reviewed_by: "reviewer@example.com",
              review_note: "Ship it",
              created_at: 1_713_600_100,
              reviewed_at: 1_713_600_300,
              executed_at: 1_713_600_301,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              runtime_mode: "queue",
              queued_tasks: 10,
              processing_tasks: 2,
              failed_documents: 1,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              failed_documents: 1,
              recent_guard_events: 2,
              share_access_events: 3,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              items: [
                {
                  id: 1,
                  actor_id: "user-1",
                  action: "task_failed",
                  resource_type: "document",
                  resource_id: "doc-1",
                  owner_user_id: "owner-1",
                  created_at: 1_713_600_400,
                },
              ],
              total: 1,
              page: 2,
              per_page: 50,
            },
            error: null,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      )
      .mockResolvedValueOnce(new Response("id,action\n1,task_failed\n", { status: 200 }));

    await expect(getAdminBillingOverview("token-123")).resolves.toEqual({
      active_subscriptions: 8,
      past_due_subscriptions: 1,
      unpaid_subscriptions: 2,
      canceled_subscriptions: 3,
    });

    await expect(getAdminRagHealth("token-123")).resolves.toEqual({
      failed_documents: 4,
      queued_tasks: 5,
      processing_tasks: 6,
      recent_guard_events: 7,
    });

    await expect(listAdminFeatureFlags("token-123")).resolves.toEqual([
      expect.objectContaining({
        key: "guard_output",
        category: "guard",
      }),
    ]);

    await expect(listAdminFeatureFlagChangeRequests("token-123", "pending")).resolves.toEqual([
      expect.objectContaining({
        id: "req-1",
        status: "pending",
      }),
    ]);

    await expect(requestAdminFeatureFlagChange("token-123", "guard_output", true, "Enable rollout")).resolves.toEqual(
      expect.objectContaining({
        id: "req-2",
        requested_enabled: true,
      }),
    );

    await expect(reviewAdminFeatureFlagChange("token-123", "req-1", true, "Ship it")).resolves.toEqual(
      expect.objectContaining({
        id: "req-1",
        status: "approved",
        review_note: "Ship it",
      }),
    );

    await expect(getAdminWorkerStatus("token-123")).resolves.toEqual({
      runtime_mode: "queue",
      queued_tasks: 10,
      processing_tasks: 2,
      failed_documents: 1,
    });

    await expect(getAdminDegradationStatus("token-123")).resolves.toEqual({
      failed_documents: 1,
      recent_guard_events: 2,
      share_access_events: 3,
    });

    await expect(
      listAdminAuditLogs("token-123", {
        action: "task_failed",
        page: 2,
        per_page: 50,
      }),
    ).resolves.toEqual({
      items: [
        {
          id: 1,
          actor_id: "user-1",
          action: "task_failed",
          resource_type: "document",
          resource_id: "doc-1",
          owner_user_id: "owner-1",
          created_at: 1_713_600_400,
        },
      ],
      total: 1,
      page: 2,
      per_page: 50,
    });

    await expect(
      exportAdminAuditLogsCsv("token-123", {
        action: "task_failed",
        page: 2,
        per_page: 50,
      }),
    ).resolves.toBe("id,action\n1,task_failed\n");

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/admin/billing",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/admin/rag-health",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/v1/admin/feature-flags",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "https://api.example.test/api/v1/admin/feature-flags/change-requests?status=pending",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock.mock.calls[4]?.[1]).toEqual(
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          enabled: true,
          reason: "Enable rollout",
        }),
      }),
    );
    expect(fetchMock.mock.calls[5]?.[1]).toEqual(
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          approved: true,
          review_note: "Ship it",
        }),
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      7,
      "https://api.example.test/api/v1/admin/system/workers",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      8,
      "https://api.example.test/api/v1/admin/system/degradation",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      9,
      "https://api.example.test/api/v1/admin/audit-logs?action=task_failed&page=2&per_page=50",
      expect.objectContaining({ method: "GET" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      10,
      "https://api.example.test/api/v1/admin/audit-logs?action=task_failed&page=2&per_page=50&format=csv",
      expect.objectContaining({ method: "GET" }),
    );
  });
});
