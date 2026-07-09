"use client";

import type { UiLocale } from "../../lib/ui-preferences";
import type { AdminOrgRow, AdminUserRow } from "../../lib/admin/client";
import { adminText } from "./admin-i18n";

export function formatCompactNumber(value: number) {
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }

  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}K`;
  }

  return value.toString();
}

export function formatUnixDate(value: number, locale: UiLocale) {
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
  }).format(new Date(value * 1000));
}

export function sortOrganizations(rows: AdminOrgRow[], sort: string) {
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

export function sortUsers(rows: AdminUserRow[], sort: string) {
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

export function formatCountLabel(
  locale: UiLocale,
  count: number,
  suffixKey: "organizationDetail.users" | "organizationsInAggregate",
) {
  return `${count} ${adminText(locale, suffixKey)}`;
}

export function rowBusy(orgId: string, busyOrgId: string, mutationPending: boolean) {
  return mutationPending && busyOrgId === orgId;
}

export const USAGE_PERIOD_OPTIONS = ["7d", "30d", "90d"] as const;
