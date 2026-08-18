"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import LegalFooterLinks from "@/components/legal/LegalFooterLinks";
import { getLicenseStatus } from "@/lib/desktop/tauri-license";
import { formatUiMessage } from "@/lib/i18n/messages";
import { isTauri } from "@/lib/runtime/tauri-ipc";
import { useUiPreferences } from "@/lib/ui-preferences";
import { AUTH_SESSION_COOKIE_NAME } from "../lib/auth/server-session";

function getCookie(name: string): string | null {
  if (typeof document === "undefined") {
    return null;
  }
  const match = document.cookie.split("; ").find((row) => row.startsWith(`${name}=`));
  return match ? match.split("=")[1] ?? null : null;
}

/**
 * App entry router + public SSR summary（GEO/SEO 方案 A2）。
 * 爬虫在无 JS 的 HTML 里即可读到产品价值主张；浏览器/桌面端仍由下面的
 * effect 跳转到 /dashboard 或 /login。根页面是桌面端（Tauri）/web 共用入口，
 * 冷启动跳转行为必须保持不变。
 */
export default function HomeClient() {
  const router = useRouter();
  const { locale } = useUiPreferences();
  const [label, setLabel] = useState("正在进入 Context-OS…");

  useEffect(() => {
    let cancelled = false;

    async function routeDesktop() {
      setLabel("正在启动客户端…");
      try {
        // ADR-0010: free client — license status optional; never block on activate.
        await getLicenseStatus().catch(() => undefined);
        if (cancelled) return;
        router.replace("/dashboard");
      } catch {
        if (!cancelled) {
          router.replace("/dashboard");
        }
      }
    }

    // Re-check isTauri after a short delay — internals can appear slightly after first paint.
    const run = () => {
      if (cancelled) return;
      if (isTauri()) {
        void routeDesktop();
        return;
      }
      // Second chance after tick (webview inject race)
      window.setTimeout(() => {
        if (cancelled) return;
        if (isTauri()) {
          void routeDesktop();
          return;
        }
        const hasAuthSession = getCookie(AUTH_SESSION_COOKIE_NAME) === "1";
        router.replace(hasAuthSession ? "/dashboard" : "/login");
      }, 50);
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [router]);

  const sections = [
    {
      title: formatUiMessage(locale, "home.seoSectionDocs"),
      body: formatUiMessage(locale, "home.seoBulletDocs"),
    },
    {
      title: formatUiMessage(locale, "home.seoSectionAgents"),
      body: formatUiMessage(locale, "home.seoBulletAgents"),
    },
    {
      title: formatUiMessage(locale, "home.seoSectionShare"),
      body: formatUiMessage(locale, "home.seoBulletShare"),
    },
    {
      title: formatUiMessage(locale, "home.seoSectionDiff"),
      body: formatUiMessage(locale, "home.seoBulletDiff"),
    },
  ] as const;

  return (
    <main className="app-page-shell">
      <div
        className="app-page-center"
        style={{ display: "grid", gap: "1.25rem", maxWidth: "44rem", padding: "3rem 1rem" }}
      >
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <h1 className="app-page-title">{formatUiMessage(locale, "home.seoTitle")}</h1>
          <p className="app-page-subtitle">{formatUiMessage(locale, "home.seoSubtitle")}</p>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
            {formatUiMessage(locale, "home.seoPublisher")}
          </p>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: 0 }}>
            {formatUiMessage(locale, "home.seoUpdated", { date: "2026-08-18" })}
          </p>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "13px", margin: 0 }}>
            {formatUiMessage(locale, "home.seoEvidence")}
          </p>
        </header>

        {sections.map((section) => (
          <section key={section.title} style={{ display: "grid", gap: "0.4rem" }}>
            <h2 style={{ fontSize: "1.15rem", margin: 0 }}>{section.title}</h2>
            <p style={{ color: "hsl(var(--muted-foreground))", margin: 0, lineHeight: 1.55 }}>
              {section.body}
            </p>
          </section>
        ))}

        <div className="app-button-row" style={{ flexWrap: "wrap" }}>
          <Link className="app-button-primary" href="/dashboard">
            {formatUiMessage(locale, "home.seoCtaEnter")}
          </Link>
          <Link className="app-button-secondary" href="/pricing">
            {formatUiMessage(locale, "home.seoCtaPricing")}
          </Link>
          <Link className="app-button-secondary" href="/help/api-access/agents">
            {formatUiMessage(locale, "home.seoCtaAgents")}
          </Link>
          <Link className="app-button-secondary" href="/help/faq">
            FAQ
          </Link>
          <Link className="app-button-secondary" href="/help/compare">
            选型对比
          </Link>
        </div>

        <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
          {label}
        </p>
      </div>
      <LegalFooterLinks />
    </main>
  );
}
