"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import { ContextOsMark } from "../context-os-mark";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";

type AdminNavItem = {
  href: string;
  label: {
    zh: string;
    en: string;
  };
  prefixes: string[];
};

const ADMIN_NAV_ITEMS: AdminNavItem[] = [
  {
    href: "/admin",
    label: { zh: "组织", en: "Organizations" },
    prefixes: ["/admin", "/admin/organizations", "/admin/orgs"],
  },
  {
    href: "/admin/users",
    label: { zh: "用户", en: "Users" },
    prefixes: ["/admin/users"],
  },
  {
    href: "/admin/usage",
    label: { zh: "用量", en: "Usage" },
    prefixes: ["/admin/usage"],
  },
  {
    href: "/admin/billing",
    label: { zh: "账单", en: "Billing" },
    prefixes: ["/admin/billing"],
  },
  {
    href: "/admin/health",
    label: { zh: "健康", en: "Health" },
    prefixes: ["/admin/health"],
  },
  {
    href: "/admin/rag-health",
    label: { zh: "RAG 健康", en: "RAG Health" },
    prefixes: ["/admin/rag-health"],
  },
  {
    href: "/admin/feature-flags",
    label: { zh: "功能开关", en: "Feature Flags" },
    prefixes: ["/admin/feature-flags"],
  },
  {
    href: "/admin/system/workers",
    label: { zh: "执行器", en: "Workers" },
    prefixes: ["/admin/system/workers"],
  },
  {
    href: "/admin/system/degradation",
    label: { zh: "降级", en: "Degradation" },
    prefixes: ["/admin/system/degradation"],
  },
  {
    href: "/admin/audit-logs",
    label: { zh: "审计日志", en: "Audit Logs" },
    prefixes: ["/admin/audit-logs"],
  },
];

function isActivePath(pathname: string, prefixes: string[]) {
  return prefixes.some((prefix) => {
    if (prefix === "/admin") {
      return pathname === prefix;
    }

    return pathname === prefix || pathname.startsWith(`${prefix}/`);
  });
}

export function AdminShell({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const { locale } = useUiPreferences();

  return (
    <main
      style={{
        minHeight: "100vh",
        display: "grid",
        gridTemplateColumns: "16rem minmax(0, 1fr)",
        background: "hsl(var(--surface-muted))",
      }}
    >
      <aside
        style={{
          borderRight: "1px solid hsl(var(--border))",
          background: "hsl(var(--card))",
          padding: "1.25rem 1rem",
          display: "grid",
          gap: "1rem",
          alignContent: "start",
        }}
      >
        <div style={{ display: "grid", gap: "0.6rem" }}>
          <Link href="/dashboard" style={{ display: "inline-flex", alignItems: "center", gap: "0.75rem" }}>
            <ContextOsMark />
            <div style={{ display: "grid", gap: "0.15rem" }}>
              <strong>Context OS</strong>
              <span style={{ fontSize: "0.82rem", color: "hsl(var(--muted-foreground))" }}>
                {formatUiMessage(locale, "adminShellTitle")}
              </span>
            </div>
          </Link>
        </div>
        <nav aria-label={formatUiMessage(locale, "adminNavLabel")} style={{ display: "grid", gap: "0.35rem" }}>
          {ADMIN_NAV_ITEMS.map((item) => {
            const active = isActivePath(pathname, item.prefixes);
            const labelKey =
              item.href === "/admin"
                ? "adminNavOrganizations"
                : item.href === "/admin/users"
                  ? "adminNavUsers"
                  : item.href === "/admin/usage"
                    ? "adminNavUsage"
                    : item.href === "/admin/billing"
                      ? "adminNavBilling"
                      : item.href === "/admin/health"
                        ? "adminNavHealth"
                        : item.href === "/admin/rag-health"
                          ? "adminNavRagHealth"
                          : item.href === "/admin/feature-flags"
                            ? "adminNavFeatureFlags"
                            : item.href === "/admin/system/workers"
                              ? "adminNavWorkers"
                              : item.href === "/admin/system/degradation"
                                ? "adminNavDegradation"
                                : "adminNavAuditLogs";

            return (
              <Link
                href={item.href}
                key={item.href}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "0.65rem",
                  minHeight: "2.5rem",
                  padding: "0.7rem 0.85rem",
                  borderRadius: "0.9rem",
                  border: `1px solid ${active ? "hsl(var(--primary))" : "transparent"}`,
                  background: active ? "hsl(var(--primary))" : "transparent",
                  color: active ? "hsl(var(--primary-foreground))" : "hsl(var(--foreground))",
                }}
              >
                {formatUiMessage(locale, labelKey)}
              </Link>
            );
          })}
        </nav>
      </aside>
      <section style={{ minWidth: 0, padding: "1.5rem" }}>{children}</section>
    </main>
  );
}
