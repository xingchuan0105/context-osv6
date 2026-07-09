import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createPortalSession,
  getSubscription,
  getUsage,
  getUsageLimit,
  getUserPreferences,
  listNotifications,
  listPlans,
  markNotificationRead,
  updateProfile,
  updateUserPreferences,
} from "../../lib/settings/client";

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

describe("settings client", () => {
  it("uses auth/profile/preferences/notifications endpoints", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            success: true,
            data: {
              token: "token-123",
              user: {
                id: "user-1",
                email: "owner@example.com",
                full_name: "Owner Updated",
              },
              reset_ticket: null,
            },
            error: null,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            dashboard: {
              favorite_workspace_ids: [],
              workspace_drafts: [],
              workspace_preferences: [],
              workspace_notes: [],
            },
            notifications: {
              email_enabled: true,
              product_enabled: true,
              security_enabled: true,
              weekly_digest_enabled: false,
              quiet_hours_start: null,
              quiet_hours_end: null,
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            dashboard: {
              favorite_workspace_ids: [],
              workspace_drafts: [],
              workspace_preferences: [],
              workspace_notes: [],
            },
            notifications: {
              email_enabled: true,
              product_enabled: false,
              security_enabled: true,
              weekly_digest_enabled: false,
              quiet_hours_start: "22:00",
              quiet_hours_end: "08:00",
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            notifications: [
              {
                id: "notif-1",
                org_id: "org-1",
                user_id: "user-1",
                event_type: "security_alert",
                title: "Security alert",
                body: "New login detected",
                data: {},
                read_at: null,
                created_at: "2026-04-20T10:00:00Z",
                updated_at: "2026-04-20T10:00:00Z",
              },
            ],
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            policy: {
              enabled: true,
              rolling_5h_limit_units: 1000,
              rolling_7d_limit_units: 7000,
            },
            windows: {
              rolling_5h: {
                used_units: 250,
                limit_units: 1000,
                remaining_units: 750,
                percent_used: 25,
                blocked: false,
                next_relief_at: null,
                blocked_until: null,
              },
              rolling_7d: {
                used_units: 1200,
                limit_units: 7000,
                remaining_units: 5800,
                percent_used: 17.14,
                blocked: false,
                next_relief_at: null,
                blocked_until: null,
              },
            },
            breakdown: {
              embedding_tokens: 300,
            },
            scope: {
              plan_default: {
                plan_id: "pro",
              },
            },
            has_estimated_usage: false,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(updateProfile("token-123", "Owner Updated")).resolves.toEqual({
      success: true,
      data: {
        token: "token-123",
        user: {
          id: "user-1",
          email: "owner@example.com",
          full_name: "Owner Updated",
        },
        reset_ticket: null,
      },
      error: null,
    });

    await expect(getUserPreferences("token-123")).resolves.toEqual({
      dashboard: {
        favorite_workspace_ids: [],
        workspace_drafts: [],
        workspace_preferences: [],
        workspace_notes: [],
      },
      notifications: {
        email_enabled: true,
        product_enabled: true,
        security_enabled: true,
        weekly_digest_enabled: false,
        quiet_hours_start: null,
        quiet_hours_end: null,
      },
    });

    await expect(
      updateUserPreferences("token-123", {
        dashboard: {
          favorite_workspace_ids: [],
          workspace_drafts: [],
          workspace_preferences: [],
          workspace_notes: [],
        },
        notifications: {
          email_enabled: true,
          product_enabled: false,
          security_enabled: true,
          weekly_digest_enabled: false,
          quiet_hours_start: "22:00",
          quiet_hours_end: "08:00",
        },
      }),
    ).resolves.toEqual({
      dashboard: {
        favorite_workspace_ids: [],
        workspace_drafts: [],
        workspace_preferences: [],
        workspace_notes: [],
      },
      notifications: {
        email_enabled: true,
        product_enabled: false,
        security_enabled: true,
        weekly_digest_enabled: false,
        quiet_hours_start: "22:00",
        quiet_hours_end: "08:00",
      },
    });

    await expect(listNotifications("token-123")).resolves.toEqual({
      notifications: [
        expect.objectContaining({
          id: "notif-1",
          event_type: "security_alert",
        }),
      ],
    });

    await expect(markNotificationRead("token-123", "notif-1")).resolves.toEqual({});

    await expect(getUsageLimit("token-123")).resolves.toEqual(
      expect.objectContaining({
        policy: expect.objectContaining({
          enabled: true,
        }),
      }),
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/auth/profile",
      expect.objectContaining({
        method: "PUT",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/auth/preferences",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/auth/preferences",
      expect.objectContaining({
        method: "PUT",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "https://api.example.test/api/v1/notifications",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      5,
      "https://api.example.test/api/v1/notifications/notif-1/read",
      expect.objectContaining({
        method: "POST",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      "https://api.example.test/api/auth/usage-limit",
      expect.objectContaining({
        method: "GET",
      }),
    );
  });

  it("maps billing envelopes into the Next frontend shape", async () => {
    fetchMock
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              plans: [
                {
                  plan_id: "pro",
                  name: "Pro",
                  description: "Professional tier",
                  price_label: "$29.00",
                  interval: "month",
                  checkout_available: true,
                  current: true,
                  quotas: [
                    {
                      metric_type: "embedding_tokens",
                      soft_limit: 100000,
                      hard_limit: null,
                    },
                  ],
                },
              ],
              current_plan_id: "pro",
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              usage: {
                embedding_tokens: 10,
                llm_input_tokens: 20,
                llm_output_tokens: 30,
                pages_processed: 4,
              },
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              subscription: {
                plan_id: "pro",
                status: "active",
                current_period_end: "2026-05-01T00:00:00Z",
              },
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            data: {
              url: "https://billing.example.test",
            },
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );

    await expect(listPlans("token-123")).resolves.toEqual({
      plans: [
        {
          id: "pro",
          name: "Pro",
          price: 2900,
          features: ["embedding_tokens: 100000"],
        },
      ],
    });

    await expect(getUsage("token-123")).resolves.toEqual({
      used_tokens: 60,
      limit_tokens: 0,
      used_documents: 4,
      limit_documents: 0,
    });

    await expect(getSubscription("token-123")).resolves.toEqual({
      plan_id: "pro",
      status: "active",
      current_period_end: "2026-05-01T00:00:00Z",
    });

    await expect(createPortalSession("token-123")).resolves.toEqual({
      url: "https://billing.example.test",
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "https://api.example.test/api/v1/billing/plans",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "https://api.example.test/api/v1/billing/usage",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "https://api.example.test/api/v1/billing/subscription",
      expect.objectContaining({
        method: "GET",
      }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      "https://api.example.test/api/v1/billing/portal-session",
      expect.objectContaining({
        method: "POST",
      }),
    );
  });
});
