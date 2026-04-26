"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useState } from "react";

import type { AdminOrgRow, AdminUserRow } from "../../lib/admin/client";
import { useAuth } from "../../lib/auth/context";
import { type UiLocale, useUiPreferences } from "../../lib/ui-preferences";
import {
  adminMessage,
  adminText,
  formatAdminError,
  healthStatusLabel,
  orgStatusLabel,
  planLabel,
  userRoleLabel,
} from "./admin-i18n";
import {
  ADMIN_ALL_ORGS_VALUE,
  getCombinedAdminQueryError,
  useAdminHealthQuery,
  useAdminOrganizationQuery,
  useAdminOrganizationUsageQuery,
  useAdminUsageScopeQuery,
  useAdminOrganizationsQuery as useOrganizationsQuery,
  useAdminOrganizationUsersQuery as useOrganizationUsersQuery,
  useUpdateAdminOrganizationBlockedMutation,
} from "./admin-queries";

const USAGE_PERIOD_OPTIONS = ["7d", "30d", "90d"] as const;

function formatCompactNumber(value: number) {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }

  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}K`;
  }

  return value.toString();
}

function formatUnixDate(value: number, locale: UiLocale) {
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(new Date(value * 1000));
}

function sortOrganizations(rows: AdminOrgRow[], sort: string) {
  const items = [...rows];

  items.sort((left, right) => {
    switch (sort) {
      case "name_asc":
        return left.name.localeCompare(right.name);
      case "users_desc":
        return right.user_count - left.user_count || left.name.localeCompare(right.name);
      case "notebooks_desc":
        return right.notebook_count - left.notebook_count || left.name.localeCompare(right.name);
      case "created_desc":
        return right.created_at - left.created_at || left.name.localeCompare(right.name);
      default:
        return right.query_count - left.query_count || left.name.localeCompare(right.name);
    }
  });

  return items;
}

function sortUsers(rows: AdminUserRow[], sort: string) {
  const items = [...rows];

  items.sort((left, right) => {
    switch (sort) {
      case "email_asc":
        return left.email.localeCompare(right.email);
      case "role_asc":
        return left.role.localeCompare(right.role) || left.email.localeCompare(right.email);
      case "last_active_desc":
        return (right.last_active_at ?? 0) - (left.last_active_at ?? 0) || right.created_at - left.created_at;
      default:
        return right.created_at - left.created_at || left.email.localeCompare(right.email);
    }
  });

  return items;
}

function formatCountLabel(locale: UiLocale, count: number, suffixKey: "organizationDetail.users" | "organizationsInAggregate") {
  return `${count} ${adminText(locale, suffixKey)}`;
}

function rowBusy(orgId: string, busyOrgId: string, mutationPending: boolean) {
  return mutationPending && busyOrgId === orgId;
}

export function AdminPageHeading({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <header style={{ display: "grid", gap: "0.35rem", marginBottom: "1rem" }}>
      <h1 style={{ margin: 0, fontSize: "clamp(1.8rem, 2.5vw, 2.4rem)", lineHeight: 1.05 }}>{title}</h1>
      <p style={{ margin: 0, color: "hsl(var(--muted-foreground))" }}>{subtitle}</p>
    </header>
  );
}

export function AdminMetricCard({
  label,
  value,
  tone = "primary",
  detail,
}: {
  label: string;
  value: string;
  tone?: "primary" | "success" | "warning" | "danger";
  detail?: string;
}) {
  const palette =
    tone === "success"
      ? { dot: "hsl(var(--success))", value: "hsl(var(--success))" }
      : tone === "warning"
        ? { dot: "hsl(var(--warning))", value: "hsl(var(--warning))" }
        : tone === "danger"
          ? { dot: "hsl(var(--destructive))", value: "hsl(var(--destructive))" }
          : { dot: "hsl(var(--info))", value: "hsl(var(--foreground))" };

  return (
    <section className="app-inline-surface" style={{ display: "grid", gap: "0.6rem" }}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "0.45rem", fontSize: "0.78rem", color: "hsl(var(--muted-foreground))" }}>
        <span style={{ width: "0.6rem", height: "0.6rem", borderRadius: "999px", background: palette.dot }} />
        <span>{label}</span>
      </div>
      <strong style={{ fontSize: "1.5rem", color: palette.value }}>{value}</strong>
      {detail ? <span style={{ fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>{detail}</span> : null}
    </section>
  );
}

export function LoadingState({ copy }: { copy: string }) {
  return (
    <section className="app-inline-surface" style={{ textAlign: "center", color: "hsl(var(--muted-foreground))" }}>
      {copy}
    </section>
  );
}

export function EmptyState({ copy }: { copy: string }) {
  return (
    <section
      className="app-inline-surface"
      style={{
        textAlign: "center",
        borderStyle: "dashed",
        color: "hsl(var(--muted-foreground))",
      }}
    >
      {copy}
    </section>
  );
}

export function ErrorState({ message }: { message: string }) {
  return <p className="app-notice-banner">{message}</p>;
}

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
        subtitle={adminText(locale, "organizations.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "organizations.loading")} />
      ) : organizations.length === 0 ? (
        <EmptyState copy={adminText(locale, "organizations.empty")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminMessage(locale, "admin.metrics.totalOrganizations")} tone="primary" value={organizations.length.toString()} />
            <AdminMetricCard label={adminText(locale, "organizations.activeOrganizations")} tone="success" value={activeCount.toString()} />
            <AdminMetricCard label={adminText(locale, "organizations.blockedOrganizations")} tone="danger" value={blockedCount.toString()} />
            <AdminMetricCard
              label={adminText(locale, "organizations.totalNotebooks")}
              tone="warning"
              value={totalNotebookCount.toString()}
              detail={`${adminText(locale, "organizations.usersCovered")} ${totalUserCount}`}
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
                  placeholder={adminText(locale, "organizations.filterByNameIdPlan")}
                  type="text"
                  value={query}
                />
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-org-status-filter">
                  {adminText(locale, "organizations.statusFilterLabel")}
                </label>
                <select
                  className="app-input"
                  id="admin-org-status-filter"
                  onChange={(event) => setStatusFilter(event.target.value)}
                  value={statusFilter}
                >
                  <option value="all">{adminText(locale, "common.allStatuses")}</option>
                  <option value="active">{adminMessage(locale, "admin.status.active")}</option>
                  <option value="blocked">{adminMessage(locale, "admin.status.blocked")}</option>
                </select>
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-org-sort">
                  {adminMessage(locale, "admin.filter.sortLabel")}
                </label>
                <select className="app-input" id="admin-org-sort" onChange={(event) => setSortMode(event.target.value)} value={sortMode}>
                  <option value="queries_desc">{adminText(locale, "organizations.sort.queriesDesc")}</option>
                  <option value="users_desc">{adminText(locale, "organizations.sort.usersDesc")}</option>
                  <option value="notebooks_desc">{adminText(locale, "organizations.sort.notebooksDesc")}</option>
                  <option value="created_desc">{adminText(locale, "users.newestFirst")}</option>
                  <option value="name_asc">{adminText(locale, "organizations.sort.nameAsc")}</option>
                </select>
              </div>
            </div>
            <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
              <span>{adminText(locale, "organizations.matching")} {filteredOrganizations.length}/{organizations.length}</span>
              <span>{adminText(locale, "organizations.usersCovered")} {filteredOrganizations.reduce((total, organization) => total + organization.user_count, 0)}</span>
            </div>
          </section>

          {filteredOrganizations.length === 0 ? (
            <EmptyState copy={adminText(locale, "organizations.noMatch")} />
          ) : (
            <section className="app-inline-surface" style={{ overflowX: "auto", padding: 0 }}>
              <table style={{ width: "100%", borderCollapse: "collapse" }}>
                <thead style={{ background: "hsl(var(--surface-muted))" }}>
                  <tr>
                    {[
                      adminText(locale, "common.name"),
                      adminMessage(locale, "admin.table.plan"),
                      adminMessage(locale, "admin.table.users"),
                      adminText(locale, "common.notebooks"),
                      adminMessage(locale, "admin.table.requests"),
                      adminMessage(locale, "admin.table.status"),
                      adminText(locale, "common.actions"),
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
                              ? adminText(locale, "common.processing")
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
  const notebooksPerUser = organization ? Math.floor(organization.notebook_count / Math.max(organization.user_count, 1)) : 0;

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
              <AdminMetricCard label={adminMessage(locale, "admin.table.plan")} value={planLabel(locale, organization.plan)} />
              <AdminMetricCard label={adminMessage(locale, "admin.table.users")} value={organization.user_count.toString()} />
              <AdminMetricCard
                label={adminText(locale, "common.notebooks")}
                value={organization.notebook_count.toString()}
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
                        adminMessage(locale, "admin.filter.roleLabel"),
                        adminMessage(locale, "admin.table.createdAt"),
                        adminMessage(locale, "admin.table.lastActive"),
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

export function AdminUsersSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const organizationsQuery = useOrganizationsQuery(actorId, token);
  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [roleFilter, setRoleFilter] = useState("all");
  const [sortMode, setSortMode] = useState("created_desc");
  const organizations = organizationsQuery.data ?? [];
  const effectiveSelectedOrgId = selectedOrgId ?? organizations[0]?.id ?? "";
  const usersQuery = useOrganizationUsersQuery(actorId, token, effectiveSelectedOrgId);
  const users = usersQuery.data ?? [];
  const filteredUsers = sortUsers(
    users.filter((user) => {
      const normalizedQuery = query.trim().toLowerCase();

      if (roleFilter !== "all" && user.role !== roleFilter) {
        return false;
      }

      if (!normalizedQuery) {
        return true;
      }

      return (
        user.email.toLowerCase().includes(normalizedQuery) ||
        user.full_name.toLowerCase().includes(normalizedQuery) ||
        user.role.toLowerCase().includes(normalizedQuery)
      );
    }),
    sortMode,
  );
  const selectedOrg = organizations.find((organization) => organization.id === effectiveSelectedOrgId) ?? null;
  const error = organizationsQuery.error ?? usersQuery.error ?? null;
  const organizationsLoading = Boolean(token) && organizationsQuery.isPending;
  const usersLoading = Boolean(token && effectiveSelectedOrgId) && usersQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminMessage(locale, "admin.nav.users")}
        subtitle={adminText(locale, "users.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(4, minmax(0, 1fr))" }}>
          <div>
            <label className="app-form-label" htmlFor="admin-users-org">
              {adminMessage(locale, "admin.table.organization")}
            </label>
            <select
              className="app-input"
              disabled={organizationsLoading || organizations.length === 0}
              id="admin-users-org"
              onChange={(event) => setSelectedOrgId(event.target.value)}
              value={effectiveSelectedOrgId}
            >
              <option value="">{adminText(locale, "common.selectOrganization")}</option>
              {organizations.map((organization) => (
                <option key={organization.id} value={organization.id}>
                  {organization.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-users-query">
              {adminMessage(locale, "admin.searchLabel")}
            </label>
            <input
              className="app-input"
              disabled={!effectiveSelectedOrgId}
              id="admin-users-query"
              onChange={(event) => setQuery(event.target.value)}
              placeholder={adminText(locale, "users.filterPlaceholder")}
              type="text"
              value={query}
            />
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-users-role">
              {adminMessage(locale, "admin.filter.roleLabel")}
            </label>
            <select className="app-input" disabled={!effectiveSelectedOrgId} id="admin-users-role" onChange={(event) => setRoleFilter(event.target.value)} value={roleFilter}>
              <option value="all">{adminText(locale, "users.allRoles")}</option>
              <option value="owner">{userRoleLabel(locale, "owner")}</option>
              <option value="admin">{userRoleLabel(locale, "admin")}</option>
              <option value="member">{userRoleLabel(locale, "member")}</option>
              <option value="editor">{userRoleLabel(locale, "editor")}</option>
              <option value="viewer">{userRoleLabel(locale, "viewer")}</option>
            </select>
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-users-sort">
              {adminMessage(locale, "admin.filter.sortLabel")}
            </label>
            <select className="app-input" disabled={!effectiveSelectedOrgId} id="admin-users-sort" onChange={(event) => setSortMode(event.target.value)} value={sortMode}>
              <option value="created_desc">{adminText(locale, "users.newestFirst")}</option>
              <option value="last_active_desc">{adminText(locale, "users.latestActive")}</option>
              <option value="email_asc">{adminText(locale, "users.sort.emailAsc")}</option>
              <option value="role_asc">{adminText(locale, "users.roleGrouping")}</option>
            </select>
          </div>
        </div>
        <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
          <span>{selectedOrg ? `${adminText(locale, "users.currentOrganization")} ${selectedOrg.name}` : adminText(locale, "users.noOrganizationSelected")}</span>
          {selectedOrg ? <span>{adminText(locale, "users.members")} {users.length}</span> : null}
        </div>
      </section>

      {!effectiveSelectedOrgId ? (
        <EmptyState copy={adminText(locale, "users.chooseOrganization")} />
      ) : usersLoading ? (
        <LoadingState copy={adminText(locale, "users.loading")} />
      ) : filteredUsers.length === 0 ? (
        <EmptyState copy={adminText(locale, "users.noMatch")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminText(locale, "common.owners")} value={users.filter((user) => user.role === "owner").length.toString()} />
            <AdminMetricCard label={adminText(locale, "common.admins")} tone="warning" value={users.filter((user) => user.role === "admin").length.toString()} />
            <AdminMetricCard label={adminText(locale, "users.memberRoles")} tone="success" value={users.filter((user) => ["member", "viewer", "editor"].includes(user.role)).length.toString()} />
            <AdminMetricCard label={adminText(locale, "common.neverActive")} tone="danger" value={users.filter((user) => user.last_active_at === null).length.toString()} />
          </div>
          <section className="app-inline-surface" style={{ overflowX: "auto", padding: 0 }}>
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead style={{ background: "hsl(var(--surface-muted))" }}>
                <tr>
                  {[
                    adminText(locale, "common.email"),
                    adminText(locale, "users.name"),
                    adminMessage(locale, "admin.filter.roleLabel"),
                    adminMessage(locale, "admin.table.createdAt"),
                    adminMessage(locale, "admin.table.lastActive"),
                  ].map((heading) => (
                    <th key={heading} style={{ padding: "0.85rem 1rem", textAlign: "left", fontSize: "0.76rem", color: "hsl(var(--muted-foreground))" }}>
                      {heading}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {filteredUsers.map((user) => (
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
    </section>
  );
}

export function AdminUsageSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const organizationsQuery = useOrganizationsQuery(actorId, token);
  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(null);
  const [selectedPeriod, setSelectedPeriod] = useState<(typeof USAGE_PERIOD_OPTIONS)[number]>("30d");
  const organizations = organizationsQuery.data ?? [];
  const effectiveSelectedOrgId = selectedOrgId ?? ADMIN_ALL_ORGS_VALUE;
  const usageScopeQuery = useAdminUsageScopeQuery(
    actorId,
    token,
    organizations,
    effectiveSelectedOrgId,
    selectedPeriod,
  );
  const usage = usageScopeQuery.data?.usage ?? null;
  const error = organizationsQuery.error ?? usageScopeQuery.error ?? null;
  const warning = usageScopeQuery.data?.failedOrgNames.length
    ? `${adminMessage(locale, "admin.loadError")} ${usageScopeQuery.data.failedOrgNames.join(", ")}`
    : "";
  const selectedOrg = organizations.find((organization) => organization.id === effectiveSelectedOrgId) ?? null;
  const scopeLabel =
    effectiveSelectedOrgId === ADMIN_ALL_ORGS_VALUE
      ? adminText(locale, "usage.aggregateScope")
      : selectedOrg?.name ?? adminText(locale, "users.noOrganizationSelected");
  const usageLoading = Boolean(token) && (organizationsQuery.isPending || usageScopeQuery.isPending);

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminMessage(locale, "admin.nav.usage")}
        subtitle={adminText(locale, "usage.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {warning ? <ErrorState message={warning} /> : null}

      <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
        <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "minmax(16rem, 18rem) minmax(0, 1fr)" }}>
          <div>
            <label className="app-form-label" htmlFor="admin-usage-scope">
              {adminText(locale, "common.scope")}
            </label>
            <select
              className="app-input"
              disabled={organizationsQuery.isPending || organizations.length === 0}
              id="admin-usage-scope"
              onChange={(event) => setSelectedOrgId(event.target.value)}
              value={effectiveSelectedOrgId}
            >
              <option value={ADMIN_ALL_ORGS_VALUE}>{adminText(locale, "usage.aggregateScope")}</option>
              {organizations.map((organization) => (
                <option key={organization.id} value={organization.id}>
                  {organization.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="app-form-label">{adminMessage(locale, "admin.filter.windowLabel")}</label>
            <div className="app-button-row">
              {USAGE_PERIOD_OPTIONS.map((period) => (
                <button
                  className={selectedPeriod === period ? "app-button-primary" : "app-button-secondary"}
                  key={period}
                  type="button"
                  onClick={() => setSelectedPeriod(period)}
                >
                  {period}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
          <span>{adminText(locale, "common.currentView")}{scopeLabel}</span>
          <span>{adminText(locale, "common.timeWindow")}{selectedPeriod}</span>
          {effectiveSelectedOrgId === ADMIN_ALL_ORGS_VALUE && organizations.length > 0 ? (
            <span>{formatCountLabel(locale, organizations.length, "organizationsInAggregate")}</span>
          ) : null}
        </div>
      </section>

      {usageLoading ? (
        <LoadingState copy={adminText(locale, "usage.loading")} />
      ) : !usage ? (
        <EmptyState copy={adminText(locale, "usage.noData")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminMessage(locale, "admin.metrics.totalRequests")} tone="primary" value={formatCompactNumber(usage.total_requests)} />
            <AdminMetricCard label={adminText(locale, "common.totalTokens")} tone="success" value={formatCompactNumber(usage.total_tokens)} />
            <AdminMetricCard label={adminMessage(locale, "admin.metrics.totalDocuments")} tone="warning" value={formatCompactNumber(usage.total_documents)} />
          </div>
          <section className="app-inline-surface" style={{ display: "grid", gap: "0.7rem" }}>
            <h2 style={{ margin: 0 }}>{adminText(locale, "common.platformStatistics")}</h2>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminMessage(locale, "admin.metrics.totalRequests")}</span>
              <strong>{usage.total_requests}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.totalTokensProcessed")}</span>
              <strong>{usage.total_tokens}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.totalIndexedDocuments")}</span>
              <strong>{usage.total_documents}</strong>
            </div>
          </section>
        </>
      )}
    </section>
  );
}

export function AdminHealthSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const healthQuery = useAdminHealthQuery(actorId, token);
  const health = healthQuery.data ?? null;
  const healthy = health ? ["ok", "healthy", "ready"].includes(health.status) : false;
  const loading = Boolean(token) && healthQuery.isPending;

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminMessage(locale, "admin.health.sectionTitle")}
        subtitle={adminMessage(locale, "admin.health.sectionSubtitle")}
      />
      {healthQuery.error ? <ErrorState message={formatAdminError(locale, healthQuery.error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "common.loading")} />
      ) : !health ? (
        <EmptyState copy={adminText(locale, "common.emptyData")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminText(locale, "common.status")} tone={healthy ? "success" : "danger"} value={healthStatusLabel(locale, health.status)} />
            <AdminMetricCard label={adminText(locale, "common.service")} value={health.service} />
            <AdminMetricCard label={adminText(locale, "common.version")} tone="warning" value={health.version} />
          </div>
          <section className="app-inline-surface" style={{ display: "grid", gap: "0.7rem" }}>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.serviceStatus")}</span>
              <strong>{healthStatusLabel(locale, health.status)}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.service")}</span>
              <strong>{health.service}</strong>
            </div>
            <div className="app-inline-row" style={{ marginBottom: 0 }}>
              <span>{adminText(locale, "common.version")}</span>
              <strong>{health.version}</strong>
            </div>
          </section>
        </>
      )}
    </section>
  );
}
