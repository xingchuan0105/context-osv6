"use client";

import Link from "next/link";
import { useParams } from "next/navigation";

import { useAuth } from "../../lib/auth/context";
import { useUiPreferences } from "../../lib/ui-preferences";
import {
  adminText,
  formatAdminError,
  accountStatusLabel,
  planLabel,
  userRoleLabel,
} from "./admin-i18n";
import {
  getCombinedAdminQueryError,
  useAdminAccountQuery,
  useAdminAccountUsageQuery,
  useAdminAccountUsersQuery as useAccountUsersQuery,
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
  formatCompactNumber,
  formatCountLabel,
  formatUnixDate,
  sortUsers,
} from "./admin-utils";

import styles from "./admin-account-detail-surface.module.css";

export function AdminAccountDetailSurface() {
  const params = useParams<{ owner_user_id: string }>();
  const ownerUserId = typeof params?.owner_user_id === "string" ? params.owner_user_id : "";
  const { token, user } = useAuth();
  const actorId = user?.id;
  const { locale } = useUiPreferences();
  const accountQuery = useAdminAccountQuery(actorId, token, ownerUserId);
  const usersQuery = useAccountUsersQuery(actorId, token, ownerUserId);
  const usage7dQuery = useAdminAccountUsageQuery(actorId, token, ownerUserId, "7d");
  const usage30dQuery = useAdminAccountUsageQuery(actorId, token, ownerUserId, "30d");
  const toggleBlockedMutation = useUpdateAdminAccountBlockedMutation(actorId, token);
  const account = accountQuery.data ?? null;
  const users = usersQuery.data ?? [];
  const usage7d = usage7dQuery.data ?? null;
  const usage30d = usage30dQuery.data ?? null;
  const loading = Boolean(token && ownerUserId) && accountQuery.isPending;
  const insightLoading = Boolean(token && ownerUserId) && (usersQuery.isPending || usage7dQuery.isPending || usage30dQuery.isPending);
  const error = accountQuery.error ?? toggleBlockedMutation.error ?? null;
  const insightError = getCombinedAdminQueryError(usersQuery, usage7dQuery, usage30dQuery);
  const ownerCount = users.filter((user) => user.role === "owner").length;
  const adminCount = users.filter((user) => user.role === "admin").length;
  const memberCount = users.filter((user) => ["member", "viewer", "editor"].includes(user.role)).length;
  const recentMembers = sortUsers(users, "created_desc").slice(0, 5);
  const requestsPerUser30d = account ? Math.floor((usage30d?.total_requests ?? 0) / Math.max(account.user_count, 1)) : 0;
  const notebooksPerUser = account ? Math.floor(account.workspace_count / Math.max(account.user_count, 1)) : 0;

  async function handleToggleBlocked() {
    if (!account) {
      return;
    }

    await toggleBlockedMutation.mutateAsync({
      ownerUserId: account.id,
      blocked: !account.blocked,
    });
  }

  return (
    <section className={styles.container}>
      <div className={styles.backRow}>
        <Link className={styles.backLink} href="/admin">
          {adminText(locale, "common.back")}
        </Link>
      </div>
      <AdminPageHeading
        title={adminText(locale, "accountDetail.title")}
        subtitle={adminText(locale, "accountDetail.subtitle")}
      />
      {error ? <ErrorState message={formatAdminError(locale, error)} /> : null}
      {loading ? (
        <LoadingState copy={adminText(locale, "accountDetail.loading")} />
      ) : !account ? (
        <EmptyState copy={adminText(locale, "accountDetail.notFound")} />
      ) : (
        <>
          <section className={`app-inline-surface ${styles.card}`}>
            <div className={styles.accountHead}>
              <div className={styles.accountTitle}>
                <h2 className={styles.heading}>{account.name}</h2>
                <p className={styles.mutedText}>
                  {adminText(locale, "common.accountId")}: {account.id}
                </p>
              </div>
              <button className="app-button-ghost" disabled={toggleBlockedMutation.isPending} type="button" onClick={() => void handleToggleBlocked()}>
                {toggleBlockedMutation.isPending
                  ? adminText(locale, "common.processing")
                  : account.blocked
                    ? adminText(locale, "accounts.unblockAccount")
                    : adminText(locale, "accounts.blockAccount")}
              </button>
            </div>
            <div className={styles.metricsGrid}>
              <AdminMetricCard label={adminText(locale, "common.status")} tone={account.blocked ? "danger" : "success"} value={accountStatusLabel(locale, account.blocked)} />
              <AdminMetricCard label={adminText(locale, "admin.table.plan")} value={planLabel(locale, account.plan)} />
              <AdminMetricCard label={adminText(locale, "admin.table.users")} value={account.user_count.toString()} />
              <AdminMetricCard
                label={adminText(locale, "common.workspaces")}
                value={account.workspace_count.toString()}
                detail={`${adminText(locale, "common.created")} ${formatUnixDate(account.created_at, locale)}`}
              />
            </div>
          </section>

          {insightError ? <ErrorState message={formatAdminError(locale, insightError)} /> : null}
          {insightLoading ? (
            <LoadingState copy={adminText(locale, "accountDetail.loadingInsights")} />
          ) : (
            <>
              <div className={styles.metricsGridWide}>
                <AdminMetricCard label={adminText(locale, "common.period7dRequests")} tone="primary" value={(usage7d?.total_requests ?? 0).toString()} />
                <AdminMetricCard label={adminText(locale, "common.period30dRequests")} tone="success" value={(usage30d?.total_requests ?? 0).toString()} />
                <AdminMetricCard label={adminText(locale, "common.tokens30d")} tone="warning" value={formatCompactNumber(usage30d?.total_tokens ?? 0)} />
                <AdminMetricCard label={adminText(locale, "common.documents30d")} tone="danger" value={(usage30d?.total_documents ?? 0).toString()} />
              </div>

              <div className={styles.twoColumnGrid}>
                <section className={`app-inline-surface ${styles.panel}`}>
                  <div className={`app-inline-row ${styles.inlineRowFlat}`}>
                    <h3 className={styles.heading}>{adminText(locale, "accountDetail.teamComposition")}</h3>
                    <span className={styles.muted}>
                      {formatCountLabel(locale, users.length, "accountDetail.users")}
                    </span>
                  </div>
                  <div className={styles.rolesGrid}>
                    <AdminMetricCard label={adminText(locale, "common.owners")} value={ownerCount.toString()} />
                    <AdminMetricCard label={adminText(locale, "common.admins")} value={adminCount.toString()} tone="warning" />
                    <AdminMetricCard label={adminText(locale, "users.memberRoles")} value={memberCount.toString()} tone="success" />
                  </div>
                  <div className={styles.memberList}>
                    {recentMembers.map((user) => (
                      <div className={`app-inline-row ${styles.inlineRowFlat}`} key={user.id}>
                        <div className={styles.memberMeta}>
                          <strong>{user.email}</strong>
                          <span className={styles.smallMuted}>
                            {userRoleLabel(locale, user.role)} · {formatUnixDate(user.created_at, locale)}
                          </span>
                        </div>
                        <span className={styles.smallMuted}>
                          {user.last_active_at ? formatUnixDate(user.last_active_at, locale) : adminText(locale, "common.neverActive")}
                        </span>
                      </div>
                    ))}
                  </div>
                </section>

                <section className={`app-inline-surface ${styles.panel}`}>
                  <h3 className={styles.heading}>{adminText(locale, "accountDetail.operationalEfficiency")}</h3>
                  <div className={styles.efficiencyGrid}>
                    <AdminMetricCard label={adminText(locale, "common.requestsPerUser30d")} value={requestsPerUser30d.toString()} />
                    <AdminMetricCard label={adminText(locale, "accountDetail.workspacesPerUser")} value={notebooksPerUser.toString()} tone="success" />
                  </div>
                  <div className={`app-inline-surface ${styles.statPanel}`}>
                    <div className={`app-inline-row ${styles.inlineRowFlat}`}>
                      <span>{adminText(locale, "common.period7dRequests")}</span>
                      <strong>{usage7d?.total_requests ?? 0}</strong>
                    </div>
                    <div className={`app-inline-row ${styles.inlineRowFlat}`}>
                      <span>{adminText(locale, "common.tokens30d")}</span>
                      <strong>{formatCompactNumber(usage30d?.total_tokens ?? 0)}</strong>
                    </div>
                    <div className={`app-inline-row ${styles.inlineRowFlat}`}>
                      <span>{adminText(locale, "common.documents30d")}</span>
                      <strong>{formatCompactNumber(usage30d?.total_documents ?? 0)}</strong>
                    </div>
                  </div>
                </section>
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
                    {sortUsers(users, "created_desc").map((user) => (
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
        </>
      )}
    </section>
  );
}
