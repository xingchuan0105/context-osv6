"use client";

import Link from "next/link";
import { useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminText,
  formatAdminError,
  accountStatusLabel,
  planLabel,
} from "./admin-i18n";
import {
  useAdminAccountsQuery as useAccountsQuery,
  useUpdateAdminAccountBlockedMutation,
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
  sortAccounts,
} from "./admin-utils";

export function AdminAccountsSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const accountsQuery = useAccountsQuery(actorId, token);
  const toggleBlockedMutation = useUpdateAdminAccountBlockedMutation(actorId, token);
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState("all");
  const [sortMode, setSortMode] = useState("queries_desc");
  const [busyOrgId, setBusyOrgId] = useState("");
  const accounts = accountsQuery.data ?? [];
  const filteredAccounts = sortAccounts(
    accounts.filter((account) => {
      const normalizedQuery = query.trim().toLowerCase();

      if (
        normalizedQuery &&
        !account.name.toLowerCase().includes(normalizedQuery) &&
        !account.id.toLowerCase().includes(normalizedQuery) &&
        !account.plan.toLowerCase().includes(normalizedQuery)
      ) {
        return false;
      }

      if (statusFilter === "active") {
        return !account.blocked;
      }

      if (statusFilter === "blocked") {
        return account.blocked;
      }

      return true;
    }),
    sortMode,
  );
  const blockedCount = accounts.filter((account) => account.blocked).length;
  const activeCount = accounts.length - blockedCount;
  const totalUserCount = accounts.reduce((total, account) => total + account.user_count, 0);
  const totalWorkspaceCount = accounts.reduce((total, account) => total + account.workspace_count, 0);
  const error = accountsQuery.error ?? toggleBlockedMutation.error ?? null;
  const loading = Boolean(token) && accountsQuery.isPending;

  async function handleToggleBlocked(ownerUserId: string, blocked: boolean) {
    setBusyOrgId(ownerUserId);

    try {
      await toggleBlockedMutation.mutateAsync({ ownerUserId, blocked });
    } finally {
      setBusyOrgId("");
    }
  }

  return (
    <section style={{ display: "grid", gap: "1rem" }}>
      <AdminPageHeading
        title={adminText(locale, "admin.nav.accounts")}
        subtitle={adminText(locale, "accounts.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "accounts.loading")} />
      ) : accounts.length === 0 ? (
        <EmptyState copy={adminText(locale, "accounts.empty")} />
      ) : (
        <>
          <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "repeat(auto-fit, minmax(12rem, 1fr))" }}>
            <AdminMetricCard label={adminText(locale, "admin.metrics.totalAccounts")} tone="primary" value={accounts.length.toString()} />
            <AdminMetricCard label={adminText(locale, "accounts.activeAccounts")} tone="success" value={activeCount.toString()} />
            <AdminMetricCard label={adminText(locale, "accounts.blockedAccounts")} tone="danger" value={blockedCount.toString()} />
            <AdminMetricCard
              label={adminText(locale, "accounts.totalWorkspaces")}
              tone="warning"
              value={totalWorkspaceCount.toString()}
              detail={`${adminText(locale, "accounts.usersCovered")} ${totalUserCount}`}
            />
          </div>

          <section className="app-inline-surface" style={{ display: "grid", gap: "0.8rem" }}>
            <div style={{ display: "grid", gap: "0.8rem", gridTemplateColumns: "minmax(0, 1fr) repeat(2, minmax(12rem, 14rem))" }}>
              <div>
                <label className="app-form-label" htmlFor="admin-org-search">
                  {adminText(locale, "admin.searchLabel")}
                </label>
                <input
                  className="app-input"
                  id="admin-org-search"
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={adminText(locale, "accounts.filterByNameIdPlan")}
                  type="text"
                  value={query}
                />
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-org-status-filter">
                  {adminText(locale, "accounts.statusFilterLabel")}
                </label>
                <select
                  className="app-input"
                  id="admin-org-status-filter"
                  onChange={(event) => setStatusFilter(event.target.value)}
                  value={statusFilter}
                >
                  <option value="all">{adminText(locale, "common.allStatuses")}</option>
                  <option value="active">{adminText(locale, "admin.status.active")}</option>
                  <option value="blocked">{adminText(locale, "admin.status.blocked")}</option>
                </select>
              </div>
              <div>
                <label className="app-form-label" htmlFor="admin-org-sort">
                  {adminText(locale, "admin.filter.sortLabel")}
                </label>
                <select className="app-input" id="admin-org-sort" onChange={(event) => setSortMode(event.target.value)} value={sortMode}>
                  <option value="queries_desc">{adminText(locale, "accounts.sort.queriesDesc")}</option>
                  <option value="users_desc">{adminText(locale, "accounts.sort.usersDesc")}</option>
                  <option value="workspaces_desc">{adminText(locale, "accounts.sort.workspacesDesc")}</option>
                  <option value="created_desc">{adminText(locale, "users.newestFirst")}</option>
                  <option value="name_asc">{adminText(locale, "accounts.sort.nameAsc")}</option>
                </select>
              </div>
            </div>
            <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
              <span>{adminText(locale, "accounts.matching")} {filteredAccounts.length}/{accounts.length}</span>
              <span>{adminText(locale, "accounts.usersCovered")} {filteredAccounts.reduce((total, account) => total + account.user_count, 0)}</span>
            </div>
          </section>

          {filteredAccounts.length === 0 ? (
            <EmptyState copy={adminText(locale, "accounts.noMatch")} />
          ) : (
            <section className="app-inline-surface" style={{ overflowX: "auto", padding: 0 }}>
              <table style={{ width: "100%", borderCollapse: "collapse" }}>
                <thead style={{ background: "hsl(var(--surface-muted))" }}>
                  <tr>
                    {[
                      adminText(locale, "common.name"),
                      adminText(locale, "admin.table.plan"),
                      adminText(locale, "admin.table.users"),
                      adminText(locale, "common.workspaces"),
                      adminText(locale, "admin.table.requests"),
                      adminText(locale, "admin.table.status"),
                      adminText(locale, "common.actions"),
                    ].map((heading) => (
                      <th key={heading} style={{ padding: "0.85rem 1rem", textAlign: "left", fontSize: "0.76rem", color: "hsl(var(--muted-foreground))" }}>
                        {heading}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {filteredAccounts.map((account) => {
                    const isBusy = rowBusy(account.id, busyOrgId, toggleBlockedMutation.isPending);

                    return (
                      <tr key={account.id} style={{ borderTop: "1px solid hsl(var(--border))" }}>
                        <td style={{ padding: "1rem" }}>
                          <Link href={`/admin/accounts/${account.id}`} style={{ fontWeight: 600 }}>
                            {account.name}
                          </Link>
                          <div style={{ fontSize: "0.78rem", color: "hsl(var(--muted-foreground))", marginTop: "0.2rem" }}>
                            ID: {account.id.slice(0, 8)}...
                          </div>
                        </td>
                        <td style={{ padding: "1rem" }}>{planLabel(locale, account.plan)}</td>
                        <td style={{ padding: "1rem" }}>{account.user_count}</td>
                        <td style={{ padding: "1rem" }}>{account.workspace_count}</td>
                        <td style={{ padding: "1rem" }}>{account.query_count}</td>
                        <td style={{ padding: "1rem" }}>
                          <span
                            style={{
                              display: "inline-flex",
                              alignItems: "center",
                              padding: "0.25rem 0.6rem",
                              borderRadius: "999px",
                              background: account.blocked ? "rgba(197, 48, 48, 0.1)" : "rgba(25, 135, 84, 0.1)",
                              color: account.blocked ? "hsl(var(--destructive))" : "hsl(var(--success))",
                            }}
                          >
                            {accountStatusLabel(locale, account.blocked)}
                          </span>
                        </td>
                        <td style={{ padding: "1rem" }}>
                          <button
                            className="app-button-ghost"
                            disabled={isBusy}
                            type="button"
                            onClick={() => void handleToggleBlocked(account.id, !account.blocked)}
                          >
                            {isBusy
                              ? adminText(locale, "common.processing")
                              : account.blocked
                                ? adminText(locale, "admin.unblockAction")
                                : adminText(locale, "admin.blockAction")}
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
