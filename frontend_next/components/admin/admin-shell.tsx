"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

import { ContextOsMark } from "../context-os-mark";
import { formatUiMessage } from "../../lib/i18n/messages";
import { useUiPreferences } from "../../lib/ui-preferences";

import styles from "./admin-shell.module.css";

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
    // Product is B2C personal: account list, not org/tenant admin.
    label: { zh: "账户", en: "Accounts" },
    prefixes: ["/admin", "/admin/accounts"],
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
    <main className={styles.main}>
      <aside className={styles.sidebar}>
        <div className={styles.brandRow}>
          <Link className={styles.brandLink} href="/dashboard">
            <ContextOsMark size={28} />
            <div className={styles.brandText}>
              <strong>Context OS</strong>
              <span className={styles.brandSubtitle}>
                {formatUiMessage(locale, "adminShellTitle")}
              </span>
            </div>
          </Link>
        </div>
        <nav aria-label={formatUiMessage(locale, "adminNavLabel")} className={styles.nav}>
          {ADMIN_NAV_ITEMS.map((item) => {
            const active = isActivePath(pathname, item.prefixes);
            const labelKey =
              item.href === "/admin"
                ? "adminNavAccounts"
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
                className={active ? `${styles.navLink} ${styles.navLinkActive}` : styles.navLink}
                href={item.href}
                key={item.href}
              >
                {formatUiMessage(locale, labelKey)}
              </Link>
            );
          })}
        </nav>
      </aside>
      <section className={styles.content}>{children}</section>
    </main>
  );
}
