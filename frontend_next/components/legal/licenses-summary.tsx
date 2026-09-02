import Link from "next/link";

import LegalLayout from "@/components/legal/LegalLayout";
import type { UiLocale } from "@/lib/i18n/config";

type LicensesSummaryCopy = {
  title: string;
  lastUpdated: string;
  overviewTitle: string;
  overviewBody: string;
  viewMit: string;
  majorTitle: string;
  tableHead: [string, string, string];
  majorComponents: { category: string; components: string; license: string }[];
  copyleftTitle: string;
  weakCopyleft: { component: string; note: string }[];
  fullTitle: string;
  viewFull: string;
  downloadMd: string;
  desktopTitle: string;
  desktopBody: string;
};

const copy: Record<UiLocale, LicensesSummaryCopy> = {
  "zh-CN": {
    title: "开源软件说明",
    lastUpdated: "2026-08-05",
    overviewTitle: "我们的产品",
    overviewBody: "Context-OS服务端与Web客户端以自研为主；整体分发遵守MIT许可证。",
    viewMit: "查看MIT许可证全文",
    majorTitle: "主要开源组件",
    tableHead: ["类别", "代表组件", "许可证"],
    majorComponents: [
      { category: "Web框架", components: "Next.js, React", license: "MIT" },
      { category: "后端运行时", components: "Tokio, Axum", license: "MIT / Apache-2.0" },
      { category: "向量检索", components: "Milvus · pgvector", license: "Apache-2.0 / PostgreSQL" },
      { category: "文档解析", components: "markitdown · firecrawl-anydoc", license: "MIT" },
      { category: "客户端壳", components: "Tauri 2", license: "MIT / Apache-2.0" },
      { category: "AI推理", components: "DeepSeek, DashScope 等", license: "商业API" },
    ],
    copyleftTitle: "弱copyleft说明",
    weakCopyleft: [
      { component: "dompurify", note: "选择Apache-2.0版本" },
      { component: "cssparser", note: "MPL，未修改则仅需NOTICE" },
      {
        component: "MinIO / Redis 7.4+（服务端）",
        note: "见第三方声明商业清单：优先云 S3/OSS；Redis 用 Valkey 或 ≤7.2",
      },
    ],
    fullTitle: "完整清单",
    viewFull: "查看完整第三方声明",
    downloadMd: "下载Markdown",
    desktopTitle: "客户端",
    desktopBody:
      "客户端壳层使用 Tauri 2（MIT / Apache-2.0）。完整安装包可捆绑便携 PostgreSQL、pgvector 与 Redis Windows 端口（BSD-3-Clause 历史端口，非 SSPL），声明见安装目录 runtime/THIRD_PARTY.txt，以及完整第三方声明中的 Desktop 章节。About 对话框亦可查看摘要。",
  },
  en: {
    title: "Open-source notices",
    lastUpdated: "2026-08-05",
    overviewTitle: "Our product",
    overviewBody:
      "The Context-OS server and web client are primarily built in-house; distribution follows the MIT license.",
    viewMit: "View the full MIT license",
    majorTitle: "Major open-source components",
    tableHead: ["Category", "Components", "License"],
    majorComponents: [
      { category: "Web framework", components: "Next.js, React", license: "MIT" },
      { category: "Backend runtime", components: "Tokio, Axum", license: "MIT / Apache-2.0" },
      { category: "Vector search", components: "Milvus · pgvector", license: "Apache-2.0 / PostgreSQL" },
      { category: "Document parsing", components: "markitdown · firecrawl-anydoc", license: "MIT" },
      { category: "Client shell", components: "Tauri 2", license: "MIT / Apache-2.0" },
      { category: "AI inference", components: "DeepSeek, DashScope, etc.", license: "Commercial API" },
    ],
    copyleftTitle: "Weak-copyleft notes",
    weakCopyleft: [
      { component: "dompurify", note: "Use the Apache-2.0 build" },
      { component: "cssparser", note: "MPL; unmodified use only requires a NOTICE" },
      {
        component: "MinIO / Redis 7.4+ (server)",
        note: "See the commercial list in third-party notices: prefer cloud S3/OSS; use Valkey or Redis ≤7.2",
      },
    ],
    fullTitle: "Full list",
    viewFull: "View full third-party notices",
    downloadMd: "Download Markdown",
    desktopTitle: "Desktop client",
    desktopBody:
      "The client shell uses Tauri 2 (MIT / Apache-2.0). Full installers may bundle portable PostgreSQL, pgvector, and Redis Windows ports (BSD-3-Clause historical ports, not SSPL); see runtime/THIRD_PARTY.txt in the install directory and the Desktop section of the full third-party notices. The About dialog also shows a summary.",
  },
};

export function LicensesSummary({ locale }: { locale: UiLocale }) {
  const t = copy[locale];

  return (
    <LegalLayout title={t.title} lastUpdated={t.lastUpdated}>
      <div className="licenses-summary">
        <section className="licenses-overview">
          <h2>{t.overviewTitle}</h2>
          <p>{t.overviewBody}</p>
          <Link href="/legal/licenses/project" className="app-link">
            {t.viewMit}
          </Link>
        </section>

        <section className="licenses-major">
          <h2>{t.majorTitle}</h2>
          <table className="licenses-table">
            <thead>
              <tr>
                <th>{t.tableHead[0]}</th>
                <th>{t.tableHead[1]}</th>
                <th>{t.tableHead[2]}</th>
              </tr>
            </thead>
            <tbody>
              {t.majorComponents.map((item, index) => (
                <tr key={index}>
                  <td>{item.category}</td>
                  <td>{item.components}</td>
                  <td><span className="license-badge">{item.license}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section className="licenses-copyleft">
          <h2>{t.copyleftTitle}</h2>
          <ul>
            {t.weakCopyleft.map((item, index) => (
              <li key={index}>
                <strong>{item.component}</strong>: {item.note}
              </li>
            ))}
          </ul>
        </section>

        <section className="licenses-full">
          <h2>{t.fullTitle}</h2>
          <div className="licenses-actions">
            <Link href="/legal/licenses/third-party" className="app-button-primary">
              {t.viewFull}
            </Link>
            <a
              href="/legal/third-party-notices.md"
              download
              className="app-button-secondary"
            >
              {t.downloadMd}
            </a>
          </div>
        </section>

        <section className="licenses-desktop">
          <h2>{t.desktopTitle}</h2>
          <p>{t.desktopBody}</p>
        </section>
      </div>
    </LegalLayout>
  );
}
