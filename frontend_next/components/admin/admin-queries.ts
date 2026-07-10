"use client";

import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseQueryResult,
} from "@tanstack/react-query";

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
  type AdminAuditLogQuery,
  type AdminAccountRow,
  type AdminUsageResponse,
} from "../../lib/admin/client";

export const ADMIN_ALL_ORGS_VALUE = "__all__";

function adminActorScope(actorId: string | null | undefined) {
  return actorId ?? "__anonymous__";
}

export const adminQueryKeys = {
  all: (actorId: string | null | undefined) => ["admin", adminActorScope(actorId)] as const,
  accounts: (actorId: string | null | undefined) =>
    [...adminQueryKeys.all(actorId), "accounts"] as const,
  account: (actorId: string | null | undefined, ownerUserId: string) =>
    [...adminQueryKeys.all(actorId), "account", ownerUserId] as const,
  accountUsers: (actorId: string | null | undefined, ownerUserId: string) =>
    [...adminQueryKeys.all(actorId), "account-users", ownerUserId] as const,
  accountUsage: (actorId: string | null | undefined, ownerUserId: string, period: string) =>
    [...adminQueryKeys.all(actorId), "account-usage", ownerUserId, period] as const,
  usageScope: (
    actorId: string | null | undefined,
    scope: string,
    period: string,
    ownerUserIds: string[],
  ) => [...adminQueryKeys.all(actorId), "usage-scope", scope, period, ...ownerUserIds] as const,
  health: (actorId: string | null | undefined) => [...adminQueryKeys.all(actorId), "health"] as const,
  billing: (actorId: string | null | undefined) => [...adminQueryKeys.all(actorId), "billing"] as const,
  ragHealth: (actorId: string | null | undefined) => [...adminQueryKeys.all(actorId), "rag-health"] as const,
  workers: (actorId: string | null | undefined) => [...adminQueryKeys.all(actorId), "workers"] as const,
  degradation: (actorId: string | null | undefined) =>
    [...adminQueryKeys.all(actorId), "degradation"] as const,
  featureFlags: (actorId: string | null | undefined) =>
    [...adminQueryKeys.all(actorId), "feature-flags"] as const,
  featureFlagRequestsRoot: (actorId: string | null | undefined) =>
    [...adminQueryKeys.all(actorId), "feature-flag-requests"] as const,
  featureFlagRequests: (actorId: string | null | undefined, status: string) =>
    [...adminQueryKeys.featureFlagRequestsRoot(actorId), status] as const,
  auditLogs: (actorId: string | null | undefined, query: AdminAuditLogQuery) =>
    [...adminQueryKeys.all(actorId), "audit-logs", query] as const,
};

const QUERY_OPTIONS = {
  retry: false,
} as const;

export function useAdminAccountsQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.accounts(actorId),
    queryFn: () => listAdminAccounts(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminAccountQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  ownerUserId: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.account(actorId, ownerUserId),
    queryFn: () => getAdminAccount(token as string, ownerUserId),
    enabled: Boolean(actorId && token && ownerUserId),
  });
}

export function useAdminAccountUsersQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  ownerUserId: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.accountUsers(actorId, ownerUserId),
    queryFn: () => listAdminUsersForAccount(token as string, ownerUserId),
    enabled: Boolean(actorId && token && ownerUserId),
  });
}

export function useAdminAccountUsageQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  ownerUserId: string,
  period: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.accountUsage(actorId, ownerUserId, period),
    queryFn: () => getAdminUsageForAccount(token as string, ownerUserId, period),
    enabled: Boolean(actorId && token && ownerUserId),
  });
}

export function useAdminUsageScopeQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  accounts: AdminAccountRow[],
  selectedOrgId: string,
  period: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.usageScope(
      actorId,
      selectedOrgId,
      period,
      accounts.map((account) => account.id),
    ),
    queryFn: () => loadUsageForScope(token as string, accounts, selectedOrgId, period),
    enabled: Boolean(
      actorId &&
        token &&
        selectedOrgId &&
        (selectedOrgId !== ADMIN_ALL_ORGS_VALUE || accounts.length > 0),
    ),
  });
}

export function useAdminHealthQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.health(actorId),
    queryFn: () => getAdminHealth(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminBillingOverviewQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.billing(actorId),
    queryFn: () => getAdminBillingOverview(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminRagHealthQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.ragHealth(actorId),
    queryFn: () => getAdminRagHealth(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminWorkerStatusQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.workers(actorId),
    queryFn: () => getAdminWorkerStatus(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminDegradationStatusQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.degradation(actorId),
    queryFn: () => getAdminDegradationStatus(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminFeatureFlagsQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.featureFlags(actorId),
    queryFn: () => listAdminFeatureFlags(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminFeatureFlagRequestsQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  status: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.featureFlagRequests(actorId, status),
    queryFn: () =>
      listAdminFeatureFlagChangeRequests(
        token as string,
        status === "all" ? null : status,
      ),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminAuditLogsQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  query: AdminAuditLogQuery,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.auditLogs(actorId, query),
    queryFn: () => listAdminAuditLogs(token as string, query),
    enabled: Boolean(actorId && token),
  });
}

export function useUpdateAdminAccountBlockedMutation(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ ownerUserId, blocked }: { ownerUserId: string; blocked: boolean }) =>
      updateAdminAccountBlocked(token as string, ownerUserId, blocked),
    onSuccess: (_data, variables) => {
      queryClient.setQueryData(
        adminQueryKeys.accounts(actorId),
        (current: AdminAccountRow[] | undefined) =>
          current?.map((account) =>
            account.id === variables.ownerUserId
              ? { ...account, blocked: variables.blocked }
              : account,
          ) ?? current,
      );
      queryClient.setQueryData(
        adminQueryKeys.account(actorId, variables.ownerUserId),
        (current: AdminAccountRow | null | undefined) =>
          current ? { ...current, blocked: variables.blocked } : current,
      );
    },
  });
}

export function useRequestAdminFeatureFlagChangeMutation(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      flagKey,
      requestedEnabled,
      reason,
    }: {
      flagKey: string;
      requestedEnabled: boolean;
      reason: string;
    }) => requestAdminFeatureFlagChange(token as string, flagKey, requestedEnabled, reason),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: adminQueryKeys.featureFlags(actorId) }),
        queryClient.invalidateQueries({ queryKey: adminQueryKeys.featureFlagRequestsRoot(actorId) }),
      ]);
    },
  });
}

export function useReviewAdminFeatureFlagChangeMutation(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      requestId,
      approved,
      note,
    }: {
      requestId: string;
      approved: boolean;
      note?: string;
    }) => reviewAdminFeatureFlagChange(token as string, requestId, approved, note),
    onSuccess: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: adminQueryKeys.featureFlags(actorId) }),
        queryClient.invalidateQueries({ queryKey: adminQueryKeys.featureFlagRequestsRoot(actorId) }),
      ]);
    },
  });
}

export function useExportAdminAuditLogsCsvMutation(
  _actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useMutation({
    mutationFn: (query: AdminAuditLogQuery) => exportAdminAuditLogsCsv(token as string, query),
  });
}

export function getCombinedAdminQueryError(
  ...queries: Array<UseQueryResult<unknown, Error>>
) {
  for (const query of queries) {
    if (query.error) {
      return query.error;
    }
  }

  return null;
}

async function loadUsageForScope(
  token: string,
  accounts: AdminAccountRow[],
  selectedOrgId: string,
  period: string,
) {
  if (selectedOrgId === ADMIN_ALL_ORGS_VALUE) {
    const results = await Promise.allSettled(
      accounts.map((account) =>
        getAdminUsageForAccount(token, account.id, period),
      ),
    );
    const aggregate: AdminUsageResponse = {
      total_requests: 0,
      total_tokens: 0,
      total_documents: 0,
    };
    const failedOrgNames: string[] = [];

    results.forEach((result, index) => {
      if (result.status === "fulfilled") {
        aggregate.total_requests += result.value.total_requests;
        aggregate.total_tokens += result.value.total_tokens;
        aggregate.total_documents += result.value.total_documents;
      } else if (accounts[index]) {
        failedOrgNames.push(accounts[index].name);
      }
    });

    return { usage: aggregate, failedOrgNames };
  }

  return {
    usage: await getAdminUsageForAccount(token, selectedOrgId, period),
    failedOrgNames: [] as string[],
  };
}
