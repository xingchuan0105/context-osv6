import type { CSSProperties } from "react";
import type { Metadata } from "next";
import Link from "next/link";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "常见问题 · Context OS",
  description:
    "Context OS 产品 FAQ：MCP / Agent 接入、工作区密钥边界、可分享名额、BYOK、会员与余额定价。",
  alternates: { canonical: "/help/faq" },
};

const UPDATED = "2026-08-12";

const h2: CSSProperties = {
  fontSize: "1.2rem",
  margin: "1.15rem 0 0.45rem",
  fontWeight: 400,
};

const p: CSSProperties = {
  margin: "0.35rem 0 0.65rem",
  lineHeight: 1.6,
  color: "hsl(var(--foreground))",
};

const muted: CSSProperties = {
  color: "hsl(var(--muted-foreground))",
  fontSize: "13px",
  margin: 0,
};

/** Public product FAQ — SSR for crawlers / GEO (Phase C). */
export default function HelpFaqPage() {
  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "48rem" }}>
        <header style={{ display: "grid", gap: "0.5rem" }}>
          <p className="app-page-subtitle" style={{ margin: 0 }}>
            产品事实 FAQ
          </p>
          <h1 className="app-page-title" style={{ margin: 0 }}>
            Context OS 常见问题
          </h1>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
            产品：Context OS · 品牌：ContextLM · 作者：邢川
          </p>
          <p style={muted}>页面说明更新日期：{UPDATED}</p>
          <p style={muted}>
            说明基于本站公开能力与定价文档（来源：
            <Link href="/pricing">定价</Link> ·{" "}
            <Link href="/help/api-access">API 接入</Link> ·{" "}
            <Link href="/help/api-access/agents">Agent 文档</Link>
            ）。
          </p>
          <div className="app-button-row" style={{ flexWrap: "wrap" }}>
            <Link className="app-button-secondary" href="/help/compare">
              选型对比
            </Link>
            <Link className="app-button-secondary" href="/help/api-access">
              API 接入
            </Link>
            <Link className="app-button-secondary" href="/pricing">
              定价
            </Link>
          </div>
        </header>

        <article
          className="app-surface-card"
          data-testid="help-faq-body"
          style={{ margin: 0, padding: "1.25rem 1.35rem" }}
        >
          <h2 style={h2}>Context OS 是什么？</h2>
          <p style={p}>
            Context OS 是可分享的个人知识工作区：把文档入库后可按库检索问答，也可把库开放给访客或外接
            Agent（MCP / API）。产品名 Context OS，品牌为 ContextLM。
          </p>

          <h2 style={h2}>如何用 MCP / 外接 Agent 接入？</h2>
          <p style={p}>
            在分享中心为工作区创建 API 密钥后，复制「完整接入包（Agent Pack）」粘贴到 Cursor / Claude
            等客户端即可。包内含 <code>workspace_id</code>、<code>api_base</code>、
            <code>mcp_http</code> 与密钥用法。步骤与工具边界见{" "}
            <Link href="/help/api-access/agents">Agent API 文档</Link>
            ；人类可读摘要见 <Link href="/help/api-access">API 访问</Link>。
          </p>

          <h2 style={h2}>工作区密钥能做什么、不能做什么？</h2>
          <p style={p}>
            工作区 API 密钥按库作用域，面向资料管理与知识库查询（索引 / 查询类权限由创建时勾选）。聊天与网络搜索等能力不走该密钥默认路径。建库、分享治理等用户态操作需要用户登录或单独签发的
            agent token，不能用工作区密钥代替。详见 Agent 文档中的 Authentication / Scope 章节。
          </p>

          <h2 style={h2}>会员档位与可分享名额是什么关系？</h2>
          <p style={p}>
            会员主商品是「可同时开启分享的工作区数量」：Free 3 / Plus 10 / Pro 100。客户端与仅自己使用的私有工作区始终免费。升级只增加分享名额，不自动等于模型调用额度。
          </p>

          <h2 style={h2}>余额充值与会员有何区别？</h2>
          <p style={p}>
            两者独立。余额用于平台模型调用、向量检索，以及分享页上由所有者承担的访客问答（Owner-pays）。可以只充值不升级，也可以两者都要。配置自定义
            Provider（BYOK）后，最终回答走你自己的模型额度，从而减少平台对话扣费。Qwen3.7 Flash 作为快速模型，同时用于文档索引和检索子代理，并从余额扣费。
          </p>

          <h2 style={h2}>什么是 BYOK？</h2>
          <p style={p}>
            BYOK（Bring Your Own Key）即在设置中配置自己的模型 Provider。最终回答走自有额度。Qwen3.7 Flash 作为快速模型，同时用于文档索引和检索子代理。入口：
            <Link href="/settings?tab=providers">设置 · 模型 Provider</Link>
            （需登录）；说明见 <Link href="/pricing">定价页</Link> 相关提示。
          </p>

          <h2 style={h2}>分享页访客问答谁付费？</h2>
          <p style={p}>
            分享开启后，访客在公开页上的问答成本由工作区所有者承担（Owner-pays），从所有者余额或
            BYOK 策略中结算，而不是向访客单独收费。具体以定价页与账单说明为准。
          </p>

          <h2 style={h2}>桌面客户端收费吗？</h2>
          <p style={p}>
            桌面客户端按产品叙事为免费使用；本机私有与本机 Agent 场景见{" "}
            <Link href="/desktop">客户端页</Link>
            。本机库对外分享需先发布到云端（向量导入、不重灌库），可分享名额仍受会员档位约束。
          </p>

          <h2 style={h2}>和笔记 AI / 通用 RAG 怎么选？</h2>
          <p style={p}>
            若你需要「按工作区隔离的知识库 + 可分享 + 外接 Agent（MCP）」，优先看 Context OS。若你主要在单一笔记产品内写文档并使用其内置
            AI，可能笔记套件更合适。中立对照表见 <Link href="/help/compare">选型对比</Link>。
          </p>

          <h2 style={h2}>证据与进一步阅读</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            <li>
              <Link href="/pricing">定价与充值</Link>（档位、余额、BYOK 提示）
            </li>
            <li>
              <Link href="/help/api-access">API 访问（人类）</Link>
            </li>
            <li>
              <Link href="/help/api-access/agents">Agent API 文档</Link>
            </li>
            <li>
              <Link href="/help/compare">Context OS 选型对比</Link>
            </li>
          </ul>
        </article>
      </div>
    </main>
  );
}
