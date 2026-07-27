"use client";

import { useState } from "react";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminText,
  formatAdminError,
  userRoleLabel,
} from "./admin-i18n";
import {
  useAdminAccountsQuery as useAccountsQuery,
  useAdminAccountUsersQuery as useAccountUsersQuery,
} from "./admin-queries";
import {
  AdminMetricCard,
  AdminPageHeading,
  EmptyState,
  ErrorState,
  LoadingState,
} from "./admin-shared-ui";
import {
  formatUnixDate,
  sortUsers,
} from "./admin-utils";

import styles from "./admin-users-surface.module.css";

export function AdminUsersSurface() {
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const accountsQuery = useAccountsQuery(actorId, token);
  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [roleFilter, setRoleFilter] = useState("all");
  const [sortMode, setSortMode] = useState("created_desc");
  const accounts = accountsQuery.data ?? [];
  const effectiveSelectedOrgId = selectedOrgId ?? accounts[0]?.id ?? "";
  const usersQuery = useAccountUsersQuery(actorId, token, effectiveSelectedOrgId);
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
  const selectedOrg = accounts.find((account) => account.id === effectiveSelectedOrgId) ?? null;
  const error = accountsQuery.error ?? usersQuery.error ?? null;
  const accountsLoading = Boolean(token) && accountsQuery.isPending;
  const usersLoading = Boolean(token && effectiveSelectedOrgId) && usersQuery.isPending;

  return (
    <section className={styles.container}>
      <AdminPageHeading
        title={adminText(locale, "admin.nav.users")}
        subtitle={adminText(locale, "users.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      <section className={`app-inline-surface ${styles.panel}`}>
        <div className={styles.filtersGrid}>
          <div>
            <label className="app-form-label" htmlFor="admin-users-org">
              {adminText(locale, "admin.table.account")}
            </label>
            <select
              className="app-input"
              disabled={accountsLoading || accounts.length === 0}
              id="admin-users-org"
              onChange={(event) => setSelectedOrgId(event.target.value)}
              value={effectiveSelectedOrgId}
            >
              <option value="">{adminText(locale, "common.selectAccount")}</option>
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="app-form-label" htmlFor="admin-users-query">
              {adminText(locale, "admin.searchLabel")}
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
              {adminText(locale, "admin.filter.roleLabel")}
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
              {adminText(locale, "admin.filter.sortLabel")}
            </label>
            <select className="app-input" disabled={!effectiveSelectedOrgId} id="admin-users-sort" onChange={(event) => setSortMode(event.target.value)} value={sortMode}>
              <option value="created_desc">{adminText(locale, "users.newestFirst")}</option>
              <option value="last_active_desc">{adminText(locale, "users.latestActive")}</option>
              <option value="email_asc">{adminText(locale, "users.sort.emailAsc")}</option>
              <option value="role_asc">{adminText(locale, "users.roleGrouping")}</option>
            </select>
          </div>
        </div>
        <div className={styles.metaRow}>
          <span>{selectedOrg ? `${adminText(locale, "users.currentAccount")} ${selectedOrg.name}` : adminText(locale, "users.noAccountSelected")}</span>
          {selectedOrg ? <span>{adminText(locale, "users.members")} {users.length}</span> : null}
        </div>
      </section>

      {!effectiveSelectedOrgId ? (
        <EmptyState copy={adminText(locale, "users.chooseAccount")} />
      ) : usersLoading ? (
        <LoadingState copy={adminText(locale, "users.loading")} />
      ) : filteredUsers.length === 0 ? (
        <EmptyState copy={adminText(locale, "users.noMatch")} />
      ) : (
        <>
          <div className={styles.metricsGrid}>
            <AdminMetricCard label={adminText(locale, "common.owners")} value={users.filter((user) => user.role === "owner").length.toString()} />
            <AdminMetricCard label={adminText(locale, "common.admins")} tone="warning" value={users.filter((user) => user.role === "admin").length.toString()} />
            <AdminMetricCard label={adminText(locale, "users.memberRoles")} tone="success" value={users.filter((user) => ["member", "viewer", "editor"].includes(user.role)).length.toString()} />
            <AdminMetricCard label={adminText(locale, "common.neverActive")} tone="danger" value={users.filter((user) => user.last_active_at === null).length.toString()} />
          </div>
          <section className={`app-inline-surface ${styles.tableWrapper}`}>
            <table className={styles.table}>
              <thead className={styles.tableHead}>
                <tr>
                  {[
                    adminText(locale, "common.email"),
                    adminText(locale, "users.name"),
                    adminText(locale, "admin.filter.roleLabel"),
                    adminText(locale, "admin.table.createdAt"),
                    adminText(locale, "admin.table.lastActive"),
                  ].map((heading) => (
                    <th className={styles.tableHeaderCell} key={heading}>
                      {heading}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {filteredUsers.map((user) => (
                  <tr className={styles.tableRow} key={user.id}>
                    <td className={styles.tableCell}>{user.email}</td>
                    <td className={styles.tableCell}>{user.full_name || "—"}</td>
                    <td className={styles.tableCell}>{userRoleLabel(locale, user.role)}</td>
                    <td className={styles.tableCell}>{formatUnixDate(user.created_at, locale)}</td>
                    <td className={styles.tableCell}>
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
