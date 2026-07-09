"use client";

import Link from "next/link";

import { formatUiMessage } from "../../../../lib/i18n/messages";
import { useUiPreferences } from "../../../../lib/ui-preferences";

export default function HelpWritePage() {
  const { locale } = useUiPreferences();

  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "72rem" }}>
        <header style={{ display: "grid", gap: "0.75rem" }}>
          <Link className="app-link app-link-muted" href="/help">
            {formatUiMessage(locale, "helpBackHelp")}
          </Link>
          <h1 className="app-page-title">{formatUiMessage(locale, "helpSectionWriteTitle")}</h1>
          <p className="app-page-subtitle">
            {locale === "zh-CN"
              ? "根据主题自动撰写长文，内置调研、大纲、分段写作与统计指纹精修。"
              : "Automatically writes long-form articles from a topic, with built-in research, outlining, sectioned drafting, and statistical-fingerprint refinement."}
          </p>
        </header>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <ul
            style={{
              color: "hsl(var(--muted-foreground))",
              display: "grid",
              gap: "0.75rem",
              margin: 0,
              paddingLeft: "1.2rem",
            }}
          >
            <li>{formatUiMessage(locale, "helpItemWrite1")}</li>
            <li>{formatUiMessage(locale, "helpItemWrite2")}</li>
            <li>{formatUiMessage(locale, "helpItemWrite3")}</li>
          </ul>
          <div>
            <Link className="app-link" href="/docs/write-mode.md">
              {formatUiMessage(locale, "helpItemWriteDocs")}
            </Link>
          </div>
        </section>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <h2 style={{ fontSize: "1.2rem", margin: 0 }}>
            {locale === "zh-CN" ? "用量预期" : "Usage expectations"}
          </h2>
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr>
                <th style={{ textAlign: "left", borderBottom: "1px solid hsl(var(--border))", padding: "0.5rem" }}>
                  {locale === "zh-CN" ? "指标" : "Metric"}
                </th>
                <th style={{ textAlign: "left", borderBottom: "1px solid hsl(var(--border))", padding: "0.5rem" }}>
                  {locale === "zh-CN" ? "典型范围" : "Typical range"}
                </th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td style={{ padding: "0.5rem" }}>{locale === "zh-CN" ? "LLM 调用" : "LLM calls"}</td>
                <td style={{ padding: "0.5rem" }}>10–20 / {locale === "zh-CN" ? "篇" : "article"}</td>
              </tr>
              <tr>
                <td style={{ padding: "0.5rem" }}>{locale === "zh-CN" ? "Token（全文）" : "Token (full)"}</td>
                <td style={{ padding: "0.5rem" }}>~100k–200k / {locale === "zh-CN" ? "篇" : "article"}</td>
              </tr>
              <tr>
                <td style={{ padding: "0.5rem" }}>{locale === "zh-CN" ? "墙钟" : "Wall clock"}</td>
                <td style={{ padding: "0.5rem" }}>2–5 min</td>
              </tr>
            </tbody>
          </table>
        </section>

        <section className="app-surface-card" style={{ display: "grid", gap: "1rem" }}>
          <h2 style={{ fontSize: "1.2rem", margin: 0 }}>
            {locale === "zh-CN" ? "降级说明" : "Degradation"}
          </h2>
          <p style={{ color: "hsl(var(--muted-foreground))", margin: 0 }}>
            {locale === "zh-CN"
              ? "当指纹 band 未全部通过时，文章仍会交付（软结束），并附带 validation_warning。单路调研失败时降级为单路。"
              : "When fingerprint bands are not fully satisfied, the article is still delivered (soft exit) with a validation_warning. If one research path fails, it degrades to single-path."}
          </p>
        </section>
      </div>
    </main>
  );
}