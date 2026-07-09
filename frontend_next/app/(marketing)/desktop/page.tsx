import Link from "next/link";

import styles from "@/components/desktop/desktop.module.css";

export default function DesktopProductPage() {
  return (
    <main className="app-page-shell" style={{ background: "#f6f5f4" }}>
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

          <div className="app-button-row" style={{ justifyContent: "center", marginTop: "1.25rem" }}>
            <Link href="/desktop/buy" className="app-button-primary">
              购买授权
            </Link>
            <Link href="/help" className="app-button-secondary">
              了解更多
            </Link>
          </div>
        </section>
      </div>
    </main>
  );
}
