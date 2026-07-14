"use client";

import Link from "next/link";

import {
  DesktopDownloadButton,
  DesktopReleaseDetails,
} from "@/components/desktop/DesktopDownloadButton";
import styles from "@/components/desktop/desktop.module.css";
import { MarketingShell } from "@/components/marketing-chrome";
import { brandHomeHref } from "@/components/product-chrome-footer";
import { APP_PATHS } from "@/lib/site-map";
import { formatUiMessage } from "@/lib/i18n/messages";
import { useUiPreferences } from "@/lib/ui-preferences";

export default function DesktopProductPage() {
  const { locale } = useUiPreferences();
  const hub = brandHomeHref();
  const hubExternal = /^https?:\/\//i.test(hub);

  return (
    <MarketingShell active="desktop">
      <main className="app-page-shell" style={{ background: "hsl(var(--surface-muted))" }}>
        <div className="app-page-center" style={{ maxWidth: "42rem" }}>
          <header className="app-page-heading" style={{ textAlign: "center" }}>
            <h1 className="app-page-title">AVRag Desktop</h1>
            <p className="app-page-subtitle">
              本地 AI 知识助手。自带 LLM API Key，离线优先，数据留在本机。
            </p>
          </header>

          <section className={styles.card}>
            <ul className={styles.buyFeatures}>
              <li>16+ LLM 服务商预设，含智谱 Coding Plan 一键配置</li>
              <li>本地文档索引与 RAG 检索，支持 PDF / Markdown</li>
              <li>买断制授权，v1.x 终身免费升级</li>
              <li>与 SaaS 工作区数据互通（可选同步）</li>
            </ul>

            <div
              className="app-button-row"
              style={{ justifyContent: "center", marginTop: "1.25rem", flexWrap: "wrap" }}
            >
              <DesktopDownloadButton />
              <Link href={APP_PATHS.desktopBuy} className="app-button-secondary">
                {formatUiMessage(locale, "desktop.buyCta")}
              </Link>
              <Link href={APP_PATHS.help} className="app-button-ghost">
                {formatUiMessage(locale, "desktop.learnMore")}
              </Link>
            </div>

            <DesktopReleaseDetails />
          </section>

          <section className={styles.card} style={{ marginTop: "1rem" }}>
            <h2 style={{ margin: "0 0 0.75rem", fontSize: "1.05rem" }}>
              {formatUiMessage(locale, "desktop.installTitle")}
            </h2>
            <ol style={{ margin: 0, paddingLeft: "1.25rem", color: "hsl(var(--muted-foreground))" }}>
              <li style={{ marginBottom: "0.4rem" }}>
                {formatUiMessage(locale, "desktop.installStep1")}
              </li>
              <li style={{ marginBottom: "0.4rem" }}>
                {formatUiMessage(locale, "desktop.installStep2")}
              </li>
              <li style={{ marginBottom: "0.4rem" }}>
                {formatUiMessage(locale, "desktop.installStep3")}
              </li>
            </ol>
            <p
              style={{
                margin: "0.75rem 0 0",
                fontSize: "0.85rem",
                color: "hsl(var(--subtle-foreground))",
              }}
            >
              {formatUiMessage(locale, "desktop.smartScreenHint")}
            </p>
          </section>

          <p
            style={{
              marginTop: "1.25rem",
              textAlign: "center",
              fontSize: "0.9rem",
              color: "hsl(var(--muted-foreground))",
              display: "flex",
              gap: "1rem",
              justifyContent: "center",
              flexWrap: "wrap",
            }}
          >
            {hubExternal ? (
              <a className="app-link" href={hub} rel="noopener noreferrer">
                {formatUiMessage(locale, "desktop.backToHub")}
              </a>
            ) : (
              <Link className="app-link" href={hub}>
                {formatUiMessage(locale, "desktop.backToHub")}
              </Link>
            )}
            <Link
              className="app-link"
              href={`${APP_PATHS.login}?next=${encodeURIComponent(APP_PATHS.dashboard)}`}
            >
              {formatUiMessage(locale, "desktop.openSaaS")}
            </Link>
          </p>
        </div>
      </main>
    </MarketingShell>
  );
}
