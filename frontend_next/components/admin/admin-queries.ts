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
  getAdminOrganization,
  getAdminRagHealth,
  getAdminUsageForOrganization,
  getAdminWorkerStatus,
  listAdminAuditLogs,
  listAdminFeatureFlagChangeRequests,
  listAdminFeatureFlags,
  listAdminOrganizations,
  listAdminUsersForOrganization,
  requestAdminFeatureFlagChange,
  reviewAdminFeatureFlagChange,
  updateAdminOrganizationBlocked,
  type AdminAuditLogQuery,
  type AdminOrgRow,
  type AdminUsageResponse,
} from "../../lib/admin/client";

export const ADMIN_ALL_ORGS_VALUE = "__all__";

function adminActorScope(actorId: string | null | undefined) {
  return actorId ?? "__anonymous__";
}

export const adminQueryKeys = {
  all: (actorId: string | null | undefined) => ["admin", adminActorScope(actorId)] as const,
  organizations: (actorId: string | null | undefined) =>
    [...adminQueryKeys.all(actorId), "organizations"] as const,
  organization: (actorId: string | null | undefined, orgId: string) =>
    [...adminQueryKeys.all(actorId), "organization", orgId] as const,
  organizationUsers: (actorId: string | null | undefined, orgId: string) =>
    [...adminQueryKeys.all(actorId), "organization-users", orgId] as const,
  organizationUsage: (actorId: string | null | undefined, orgId: string, period: string) =>
    [...adminQueryKeys.all(actorId), "organization-usage", orgId, period] as const,
  usageScope: (
    actorId: string | null | undefined,
    scope: string,
    period: string,
    orgIds: string[],
  ) => [...adminQueryKeys.all(actorId), "usage-scope", scope, period, ...orgIds] as const,
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

export function useAdminOrganizationsQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.organizations(actorId),
    queryFn: () => listAdminOrganizations(token as string),
    enabled: Boolean(actorId && token),
  });
}

export function useAdminOrganizationQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  orgId: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.organization(actorId, orgId),
    queryFn: () => getAdminOrganization(token as string, orgId),
    enabled: Boolean(actorId && token && orgId),
  });
}

export function useAdminOrganizationUsersQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  orgId: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.organizationUsers(actorId, orgId),
    queryFn: () => listAdminUsersForOrganization(token as string, orgId),
    enabled: Boolean(actorId && token && orgId),
  });
}

export function useAdminOrganizationUsageQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  orgId: string,
  period: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.organizationUsage(actorId, orgId, period),
    queryFn: () => getAdminUsageForOrganization(token as string, orgId, period),
    enabled: Boolean(actorId && token && orgId),
  });
}

export function useAdminUsageScopeQuery(
  actorId: string | null | undefined,
  token: string | null | undefined,
  organizations: AdminOrgRow[],
  selectedOrgId: string,
  period: string,
) {
  return useQuery({
    ...QUERY_OPTIONS,
    queryKey: adminQueryKeys.usageScope(
      actorId,
      selectedOrgId,
      period,
      organizations.map((organization) => organization.id),
    ),
    queryFn: () => loadUsageForScope(token as string, organizations, selectedOrgId, period),
    enabled: Boolean(
      actorId &&
        token &&
        selectedOrgId &&
        (selectedOrgId !== ADMIN_ALL_ORGS_VALUE || organizations.length > 0),
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

export function useUpdateAdminOrganizationBlockedMutation(
  actorId: string | null | undefined,
  token: string | null | undefined,
) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ orgId, blocked }: { orgId: string; blocked: boolean }) =>
      updateAdminOrganizationBlocked(token as string, orgId, blocked),
    onSuccess: (_data, variables) => {
      queryClient.setQueryData(
        adminQueryKeys.organizations(actorId),
        (current: AdminOrgRow[] | undefined) =>
          current?.map((organization) =>
            organization.id === variables.orgId
              ? { ...organization, blocked: variables.blocked }
              : organization,
          ) ?? current,
      );
      queryClient.setQueryData(
        adminQueryKeys.organization(actorId, variables.orgId),
        (current: AdminOrgRow | null | undefined) =>
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
  organizations: AdminOrgRow[],
  selectedOrgId: string,
  period: string,
) {
  if (selectedOrgId === ADMIN_ALL_ORGS_VALUE) {
    const results = await Promise.allSettled(
      organizations.map((organization) =>
        getAdminUsageForOrganization(token, organization.id, period),
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
      } else if (organizations[index]) {
        failedOrgNames.push(organizations[index].name);
      }
    });

    return { usage: aggregate, failedOrgNames };
  }

  return {
    usage: await getAdminUsageForOrganization(token, selectedOrgId, period),
    failedOrgNames: [] as string[],
  };
}
