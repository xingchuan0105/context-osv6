"use client";

import Link from "next/link";
import { useParams } from "next/navigation";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminText,
  formatAdminError,
  orgStatusLabel,
  planLabel,
  userRoleLabel,
} from "./admin-i18n";
import {
  getCombinedAdminQueryError,
  useAdminOrganizationQuery,
  useAdminOrganizationUsageQuery,
  useAdminOrganizationUsersQuery as useOrganizationUsersQuery,
  useUpdateAdminOrganizationBlockedMutation,
} from "./admin-queries";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "./admin-shared-ui";
import {
  formatCompactNumber,
  formatCountLabel,
  formatUnixDate,
  sortUsers,
} from "./admin-utils";

export function AdminOrganizationDetailSurface() {
  const params = useParams<{ org_id: string }>();
  const orgId = typeof params?.org_id === "string" ? params.org_id : "";
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const organizationQuery = useAdminOrganizationQuery(actorId, token, orgId);
  const usersQuery = useOrganizationUsersQuery(actorId, token, orgId);
  const usage7dQuery = useAdminOrganizationUsageQuery(actorId, token, orgId, "7d");
  const usage30dQuery = useAdminOrganizationUsageQuery(actorId, token, orgId, "30d");
  const toggleBlockedMutation = useUpdateAdminOrganizationBlockedMutation(actorId, token);
  const organization = organizationQuery.data ?? null;
  const users = usersQuery.data ?? [];
  const usage7d = usage7dQuery.data ?? null;
  const usage30d = usage30dQuery.data ?? null;
  const loading = Boolean(token && orgId) && organizationQuery.isPending;
  const insightLoading = Boolean(token && orgId) && (usersQuery.isPending || usage7dQuery.isPending || usage30dQuery.isPending);
  const error = organizationQuery.error ?? toggleBlockedMutation.error ?? null;
  const insightError = getCombinedAdminQueryError(usersQuery, usage7dQuery, usage30dQuery);
  const ownerCount = users.filter((user) => user.role === "owner").length;
  const adminCount = users.filter((user) => user.role === "admin").length;
  const memberCount = users.filter((user) => ["member", "viewer", "editor"].includes(user.role)).length;
  const recentMembers = sortUsers(users, "created_desc").slice(0, 5);
  const requestsPerUser30d = organization ? Math.floor((usage30d?.total_requests ?? 0) / Math.max(organization.user_count, 1)) : 0;
  const notebooksPerUser = organization ? Math.floor(organization.workspace_count / Math.max(organization.user_count, 1)) : 0;

  async function handleToggleBlocked() {
    if (!organization) {
      return;
    }

    await toggleBlockedMutation.mutateAsync({
      orgId: organization.id,
      blocked: !organization.blocked,
    });
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
        <Link href="/admin" style={{ color: "hsl(var(--muted-foreground))" }}>
          {adminText(locale, "common.back")}
        </Link>
      </div>
      <AdminPageHeading
        title={adminText(locale, "organizationDetail.title")}
        subtitle={adminText(locale, "organizationDetail.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "organizationDetail.loading")} />
      ) : !organization ? (
        <EmptyState copy={adminText(locale, "organizationDetail.notFound")} />
      ) : (
        <>
          <section className="app-inline-surface" style={{ display: "grid", gap: "1rem" }}>
            <div style={{ display: "flex", justifyContent: "space-between", gap: "1rem", alignItems: "start" }}>
              <div style={{ display: "grid", gap: "0.35rem" }}>
                <h2 style={{ margin: 0 }}>{organization.name}</h2>
                <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>
                  {adminText(locale, "common.organizationId")}: {organization.id}
                </p>
              </div>
              <button className="app-button-ghost" disabled={toggleBlockedMutation.isPending} type="button" onClick={() => void handleToggleBlocked()}>
                {toggleBlockedMutation.isPending
                  ? adminText(locale, "common.processing")
                  : organization.blocked
                    ? adminText(locale, "organizations.unblockOrganization")
                    : adminText(locale, "organizations.blockOrganization")}
              </button>
            </div>
            <div style={{ display: "grid", gap: "0.7rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
              <AdminMetricCard label={adminText(locale, "common.status")} tone={organization.blocked ? "danger" : "success"} value={orgStatusLabel(locale, organization.blocked)} />
              <AdminMetricCard label={adminText(locale, "admin.table.plan")} value={planLabel(locale, organization.plan)} />
              <AdminMetricCard label={adminText(locale, "admin.table.users")} value={organization.user_count.toString()} />
              <AdminMetricCard
                label={adminText(locale, "common.notebooks")}
                value={organization.workspace_count.toString()}
                detail={`${adminText(locale, "common.created")} ${formatUnixDate(organization.created_at, locale)}`}
              />
            </div>
          </section>

          {insightError ? <ErrorState message={formatAdminError(locale, insightError)} /> : null}
          {insightLoading ? (
            <LoadingState copy={adminText(locale, "organizationDetail.loadingInsights")} />
          ) : (
            <>
              <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
                <AdminMetricCard label={adminText(locale, "common.period7dRequests")} tone="primary" value={(usage7d?.total_requests ?? 0).toString()} />
                <AdminMetricCard label={adminText(locale, "common.period30dRequests")} tone="success" value={(usage30d?.total_requests ?? 0).toString()} />
                <AdminMetricCard label={adminText(locale, "common.tokens30d")} tone="warning" value={formatCompactNumber(usage30d?.total_tokens ?? 0)} />
                <AdminMetricCard label={adminText(locale, "common.documents30d")} tone="danger" value={(usage30d?.total_documents ?? 0).toString()} />
              </div>

              <div style={{ display: "grid", gap: "1rem", gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr)" }}>
                <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
                  <div className="app-inline-row" style={{ marginBottom: 0 }}>
                    <h3 style={{ margin: 0 }}>{adminText(locale, "organizationDetail.teamComposition")}</h3>
                    <span style={{ color: "hsl(var(--muted-foreground))" }}>
                      {formatCountLabel(locale, users.length, "organizationDetail.users")}
                    </span>
                  </div>
                  <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(3, minmax(0, 1fr))" }}>
                    <AdminMetricCard label={adminText(locale, "common.owners")} value={ownerCount.toString()} />
                    <AdminMetricCard label={adminText(locale, "common.admins")} value={adminCount.toString()} tone="warning" />
                    <AdminMetricCard label={adminText(locale, "users.memberRoles")} value={memberCount.toString()} tone="success" />
                  </div>
                  <div style={{ display: "grid", gap: "0.5rem" }}>
                    {recentMembers.map((user) => (
                      <div className="app-inline-row" key={user.id} style={{ marginBottom: 0 }}>
                        <div style={{ display: "grid", gap: "0.2rem" }}>
                          <strong>{user.email}</strong>
                          <span style={{ fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
                            {userRoleLabel(locale, user.role)} · {formatUnixDate(user.created_at, locale)}
                          </span>
                        </div>
                        <span style={{ fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
                          {user.last_active_at ? formatUnixDate(user.last_active_at, locale) : adminText(locale, "common.neverActive")}
                        </span>
                      </div>
                    ))}
                  </div>
                </section>

                <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
                  <h3 style={{ margin: 0 }}>{adminText(locale, "organizationDetail.operationalEfficiency")}</h3>
                  <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(2, minmax(0, 1fr))" }}>
                    <AdminMetricCard label={adminText(locale, "common.requestsPerUser30d")} value={requestsPerUser30d.toString()} />
                    <AdminMetricCard label={adminText(locale, "organizationDetail.notebooksPerUser")} value={notebooksPerUser.toString()} tone="success" />
                  </div>
                  <div className="app-inline-surface" style={{ display: "grid", gap: "0.5rem" }}>
                    <div className="app-inline-row" style={{ marginBottom: 0 }}>
                      <span>{adminText(locale, "common.period7dRequests")}</span>
                      <strong>{usage7d?.total_requests ?? 0}</strong>
                    </div>
                    <div className="app-inline-row" style={{ marginBottom: 0 }}>
                      <span>{adminText(locale, "common.tokens30d")}</span>
                      <strong>{formatCompactNumber(usage30d?.total_tokens ?? 0)}</strong>
                    </div>
                    <div className="app-inline-row" style={{ marginBottom: 0 }}>
                      <span>{adminText(locale, "common.documents30d")}</span>
                      <strong>{usage30d?.total_documents ?? 0}</strong>
                    </div>
                  </div>
                </section>
              </div>

              <section className="app-inline-surface" style={{ overflowX: "auto", padding: 0 }}>
                <table style={{ width: "100%", borderCollapse: "collapse" }}>
                  <thead style={{ background: "hsl(var(--surface-muted))" }}>
                    <tr>
                      {[
                        adminText(locale, "common.email"),
                        adminText(locale, "users.name"),
                        adminText(locale, "admin.filter.roleLabel"),
                        adminText(locale, "admin.table.createdAt"),
                        adminText(locale, "admin.table.lastActive"),
                      ].map((heading) => (
                        <th key={heading} style={{ padding: "0.85rem 1rem", textAlign: "left", fontSize: "0.76rem", color: "hsl(var(--muted-foreground))" }}>
                          {heading}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {sortUsers(users, "created_desc").map((user) => (
                      <tr key={user.id} style={{ borderTop: "1px solid hsl(var(--border))" }}>
                        <td style={{ padding: "1rem" }}>{user.email}</td>
                        <td style={{ padding: "1rem" }}>{user.full_name || "—"}</td>
                        <td style={{ padding: "1rem" }}>{userRoleLabel(locale, user.role)}</td>
                        <td style={{ padding: "1rem" }}>{formatUnixDate(user.created_at, locale)}</td>
                        <td style={{ padding: "1rem" }}>
                          {user.last_active_at ? formatUnixDate(user.last_active_at, locale) : adminText(locale, "common.never")}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </section>
            </>
          )}
        </>
      )}
    </section>
  );
}
