"use client";

import Link from "next/link";
import { useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminMessage,
  formatAdminError,
  orgStatusLabel,
  planLabel,
} from "./admin-i18n";
import {
  useAdminOrganizationsQuery as useOrganizationsQuery,
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
  rowBusy,
  sortOrganizations,
} from "./admin-utils";

export function AdminOrganizationsSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const organizationsQuery = useOrganizationsQuery(actorId, token);
  const toggleBlockedMutation = useUpdateAdminOrganizationBlockedMutation(actorId, token);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState("all");
  const [sortMode, setSortMode] = useState("queries_desc");
  const [busyOrgId, setBusyOrgId] = useState("");
  const organizations = organizationsQuery.data ?? [];
  const filteredOrganizations = sortOrganizations(
    organizations.filter((organization) => {
      const normalizedQuery = query.trim().toLowerCase();

      if (
        normalizedQuery &&
        !organization.name.toLowerCase().includes(normalizedQuery) &&
        !organization.id.toLowerCase().includes(normalizedQuery) &&
        !organization.plan.toLowerCase().includes(normalizedQuery)
      ) {
        return false;
      }

      if (statusFilter === "active") {
        return !organization.blocked;
      }

      if (statusFilter === "blocked") {
        return organization.blocked;
      }

      return true;
    }),
    sortMode,
  );
  const blockedCount = organizations.filter((organization) => organization.blocked).length;
  const activeCount = organizations.length - blockedCount;
  const totalUserCount = organizations.reduce((total, organization) => total + organization.user_count, 0);
  const totalNotebookCount = organizations.reduce((total, organization) => total + organization.notebook_count, 0);
  const error = organizationsQuery.error ?? toggleBlockedMutation.error ?? null;
  const loading = Boolean(token) && organizationsQuery.isPending;

  async function handleToggleBlocked(orgId: string, blocked: boolean) {
    setBusyOrgId(orgId);

    try {
      await toggleBlockedMutation.mutateAsync({ orgId, blocked });
    } finally {
      setBusyOrgId("");
    }
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminMessage(locale, "admin.nav.organizations")}
        subtitle={adminMessage(locale, "organizations.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {loading ? (
        <LoadingState copy={adminMessage(locale, "organizations.loading")} />
      ) : organizations.length === 0 ? (
        <EmptyState copy={adminMessage(locale, "organizations.empty")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminMessage(locale, "admin.metrics.totalOrganizations")} tone="primary" value={organizations.length.toString()} />
            <AdminMetricCard label={adminMessage(locale, "organizations.activeOrganizations")} tone="success" value={activeCount.toString()} />
            <AdminMetricCard label={adminMessage(locale, "organizations.blockedOrganizations")} tone="danger" value={blockedCount.toString()} />
            <AdminMetricCard
              label={adminMessage(locale, "organizations.totalNotebooks")}
              tone="warning"
              value={totalNotebookCount.toString()}
              detail={`${adminMessage(locale, "organizations.usersCovered")} ${totalUserCount}`}
            />
          </div>

          <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
            <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "minmax(0, 1fr) repeat(2, minmax(12rem, 14rem))" }}>
              <div>
                <label className="app-form-label" htmlFor="admin-org-search">
                  {adminMessage(locale, "admin.searchLabel")}
                </label>
                <input
                  className="app-input"
                  id="admin-org-search"
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={adminMessage(locale, "organizations.filterByNameIdPlan")}
                  type="text"
                  value={query}
                />
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-org-status-filter">
                  {adminMessage(locale, "organizations.statusFilterLabel")}
                </label>
                <select
                  className="app-input"
                  id="admin-org-status-filter"
                  onChange={(event) => setStatusFilter(event.target.value)}
                  value={statusFilter}
                >
                  <option value="all">{adminMessage(locale, "common.allStatuses")}</option>
                  <option value="active">{adminMessage(locale, "admin.status.active")}</option>
                  <option value="blocked">{adminMessage(locale, "admin.status.blocked")}</option>
                </select>
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-org-sort">
                  {adminMessage(locale, "admin.filter.sortLabel")}
                </label>
                <select className="app-input" id="admin-org-sort" onChange={(event) => setSortMode(event.target.value)} value={sortMode}>
                  <option value="queries_desc">{adminMessage(locale, "organizations.sort.queriesDesc")}</option>
                  <option value="users_desc">{adminMessage(locale, "organizations.sort.usersDesc")}</option>
                  <option value="notebooks_desc">{adminMessage(locale, "organizations.sort.notebooksDesc")}</option>
                  <option value="created_desc">{adminMessage(locale, "users.newestFirst")}</option>
                  <option value="name_asc">{adminMessage(locale, "organizations.sort.nameAsc")}</option>
                </select>
              </div>
            </div>
            <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
              <span>{adminMessage(locale, "organizations.matching")} {filteredOrganizations.length}/{organizations.length}</span>
              <span>{adminMessage(locale, "organizations.usersCovered")} {filteredOrganizations.reduce((total, organization) => total + organization.user_count, 0)}</span>
            </div>
          </section>

          {filteredOrganizations.length === 0 ? (
            <EmptyState copy={adminMessage(locale, "organizations.noMatch")} />
          ) : (
            <section className="app-inline-surface" style={{ overflowX: "auto", padding: 0 }}>
              <table style={{ width: "100%", borderCollapse: "collapse" }}>
                <thead style={{ background: "hsl(var(--surface-muted))" }}>
                  <tr>
                    {[
                      adminMessage(locale, "common.name"),
                      adminMessage(locale, "admin.table.plan"),
                      adminMessage(locale, "admin.table.users"),
                      adminMessage(locale, "common.notebooks"),
                      adminMessage(locale, "admin.table.requests"),
                      adminMessage(locale, "admin.table.status"),
                      adminMessage(locale, "common.actions"),
                    ].map((heading) => (
                      <th key={heading} style={{ padding: "0.85rem 1rem", textAlign: "left", fontSize: "0.76rem", color: "hsl(var(--muted-foreground))" }}>
                        {heading}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {filteredOrganizations.map((organization) => {
                    const isBusy = rowBusy(organization.id, busyOrgId, toggleBlockedMutation.isPending);

                    return (
                      <tr key={organization.id} style={{ borderTop: "1px solid hsl(var(--border))" }}>
                        <td style={{ padding: "1rem" }}>
                          <Link href={`/admin/organizations/${organization.id}`} style={{ fontWeight: 600 }}>
                            {organization.name}
                          </Link>
                          <div style={{ fontSize: "0.78rem", color: "hsl(var(--muted-foreground))", marginTop: "0.2rem" }}>
                            ID: {organization.id.slice(0, 8)}...
                          </div>
                        </td>
                        <td style={{ padding: "1rem" }}>{planLabel(locale, organization.plan)}</td>
                        <td style={{ padding: "1rem" }}>{organization.user_count}</td>
                        <td style={{ padding: "1rem" }}>{organization.notebook_count}</td>
                        <td style={{ padding: "1rem" }}>{organization.query_count}</td>
                        <td style={{ padding: "1rem" }}>
                          <span
                            style={{
                              display: "inline-flex",
                              alignItems: "center",
                              padding: "0.25rem 0.6rem",
                              borderRadius: "999px",
                              background: organization.blocked ? "rgba(197, 48, 48, 0.1)" : "rgba(25, 135, 84, 0.1)",
                              color: organization.blocked ? "hsl(var(--destructive))" : "hsl(var(--success))",
                            }}
                          >
                            {orgStatusLabel(locale, organization.blocked)}
                          </span>
                        </td>
                        <td style={{ padding: "1rem" }}>
                          <button
                            className="app-button-ghost"
                            disabled={isBusy}
                            type="button"
                            onClick={() => void handleToggleBlocked(organization.id, !organization.blocked)}
                          >
                            {isBusy
                              ? adminMessage(locale, "common.processing")
                              : organization.blocked
                                ? adminMessage(locale, "admin.unblockAction")
                                : adminMessage(locale, "admin.blockAction")}
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </section>
          )}
        </>
      )}
    </section>
  );
}
