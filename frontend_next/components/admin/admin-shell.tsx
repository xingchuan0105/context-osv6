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
  labelKey:
    | "adminNavAccounts"
    | "adminNavUsers"
    | "adminNavUsage"
    | "adminNavBilling"
    | "adminNavHealth"
    | "adminNavRagHealth"
    | "adminNavFeatureFlags"
    | "adminNavWorkers"
    | "adminNavDegradation"
    | "adminNavAuditLogs"
    | "adminNavBroadcast";
  prefixes: string[];
};

const ADMIN_NAV_ITEMS: AdminNavItem[] = [
  {
    href: "/admin",
    // Product is B2C personal: account list, not org/tenant admin.
    labelKey: "adminNavAccounts",
    prefixes: ["/admin", "/admin/accounts"],
  },
  {
    href: "/admin/users",
    labelKey: "adminNavUsers",
    prefixes: ["/admin/users"],
  },
  {
    href: "/admin/usage",
    labelKey: "adminNavUsage",
    prefixes: ["/admin/usage"],
  },
  {
    href: "/admin/billing",
    labelKey: "adminNavBilling",
    prefixes: ["/admin/billing"],
  },
  {
    href: "/admin/health",
    labelKey: "adminNavHealth",
    prefixes: ["/admin/health"],
  },
  {
    href: "/admin/rag-health",
    labelKey: "adminNavRagHealth",
    prefixes: ["/admin/rag-health"],
  },
  {
    href: "/admin/feature-flags",
    labelKey: "adminNavFeatureFlags",
    prefixes: ["/admin/feature-flags"],
  },
  {
    href: "/admin/system/workers",
    labelKey: "adminNavWorkers",
    prefixes: ["/admin/system/workers"],
  },
  {
    href: "/admin/system/degradation",
    labelKey: "adminNavDegradation",
    prefixes: ["/admin/system/degradation"],
  },
  {
    href: "/admin/audit-logs",
    labelKey: "adminNavAuditLogs",
    prefixes: ["/admin/audit-logs"],
  },
  {
    href: "/admin/broadcast",
    labelKey: "adminNavBroadcast",
    prefixes: ["/admin/broadcast"],
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
              <strong>Context-OS</strong>
              <span className={styles.brandSubtitle}>
                {formatUiMessage(locale, "adminShellTitle")}
              </span>
            </div>
          </Link>
        </div>
        <nav aria-label={formatUiMessage(locale, "adminNavLabel")} className={styles.nav}>
          {ADMIN_NAV_ITEMS.map((item) => {
            const active = isActivePath(pathname, item.prefixes);
            return (
              <Link
                className={active ? `${styles.navLink} ${styles.navLinkActive}` : styles.navLink}
                href={item.href}
                key={item.href}
              >
                {formatUiMessage(locale, item.labelKey)}
              </Link>
            );
          })}
        </nav>
      </aside>
      <section className={styles.content}>{children}</section>
    </main>
  );
}
