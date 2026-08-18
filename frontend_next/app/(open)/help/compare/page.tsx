import type { CSSProperties } from "react";
import type { Metadata } from "next";
import Link from "next/link";

export const dynamic = "force-static";

export const metadata: Metadata = {
  title: "选型对比 · Context OS",
  description:
    "Context OS 与笔记内置 AI、第二大脑应用、通用 RAG 套件的中立对比：适用场景、分享、外接 Agent，不编造竞品数据。",
  alternates: { canonical: "/help/compare" },
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

const th: CSSProperties = {
  textAlign: "left",
  borderBottom: "1px solid hsl(var(--border))",
  padding: "0.45rem 0.55rem",
  fontWeight: 400,
  verticalAlign: "top",
};

const td: CSSProperties = {
  borderBottom: "1px solid hsl(var(--border))",
  padding: "0.45rem 0.55rem",
  verticalAlign: "top",
  fontSize: "0.92rem",
  lineHeight: 1.5,
};

type Row = { dim: string; cos: string; notes: string; rag: string };

const ROWS: Row[] = [
  {
    dim: "核心对象",
    cos: "工作区（Workspace）为产品真相；来源 / 笔记 / 对话挂在库上",
    notes: "页面或笔记本为中心；AI 多为文档内嵌能力",
    rag: "管道 / 索引 / 应用代码为中心",
  },
  {
    dim: "入库与问答",
    cos: "上传文件或 URL 成资料源；按库检索，回答可溯源到文档",
    notes: "写作与整理强；跨库检索深度因产品而异",
    rag: "可自建任意检索栈；需自运维与调参",
  },
  {
    dim: "对外分享",
    cos: "库级分享；访客问答由所有者付费（Owner-pays）；会员控制可分享名额",
    notes: "常见为页面/空间协作；知识库「访客问答 + 所有者计费」模型不一定等同",
    rag: "需自建鉴权、配额与计费",
  },
  {
    dim: "外接 Agent",
    cos: "一等能力：工作区密钥 + MCP HTTP + Agent Pack 一次复制接入",
    notes: "部分产品提供 API/插件；MCP 工作区密钥路径并非默认主叙事",
    rag: "完全可定制；集成成本由团队承担",
  },
  {
    dim: "模型与费用",
    cos: "会员（分享名额）与余额（平台模型/检索）分离；支持 BYOK",
    notes: "多为套餐或席位；模型计费因厂商而异",
    rag: "基础设施 + 模型账单自理",
  },
  {
    dim: "客户端",
    cos: "桌面客户端按产品叙事免费；本机私有与本机 Agent 场景见客户端页",
    notes: "通常与笔记编辑体验绑定",
    rag: "自选部署形态",
  },
];

/** Neutral comparison page — SSR for crawlers / GEO (Phase C). No fabricated competitor metrics. */
export default function HelpComparePage() {
  return (
    <main className="app-page-shell">
      <div className="app-page-center" style={{ display: "grid", gap: "1rem", maxWidth: "52rem" }}>
        <header style={{ display: "grid", gap: "0.5rem" }}>
          <p className="app-page-subtitle" style={{ margin: 0 }}>
            中立选型对照
          </p>
          <h1 className="app-page-title" style={{ margin: 0 }}>
            Context OS 与其它知识方案怎么选
          </h1>
          <p style={{ color: "hsl(var(--muted-foreground))", fontSize: "14px", margin: 0 }}>
            产品：Context OS · 品牌：ContextLM · 作者：邢川
          </p>
          <p style={muted}>页面说明更新日期：{UPDATED}</p>
          <p style={muted}>
            下表描述产品形态差异，不引用未核验的竞品流量、排名或价格数字（方法：对照本站公开定价与
            API 文档，竞品侧仅写常见产品类别特征）。来源：
            <Link href="/pricing">定价</Link> ·{" "}
            <Link href="/help/api-access/agents">Agent 文档</Link> ·{" "}
            <Link href="/help/faq">FAQ</Link>。
          </p>
          <div className="app-button-row" style={{ flexWrap: "wrap" }}>
            <Link className="app-button-secondary" href="/help/faq">
              常见问题
            </Link>
            <Link className="app-button-secondary" href="/help/api-access/agents">
              Agent 接入
            </Link>
            <Link className="app-button-secondary" href="/pricing">
              定价
            </Link>
          </div>
        </header>

        <article
          className="app-surface-card"
          data-testid="help-compare-body"
          style={{ margin: 0, padding: "1.25rem 1.35rem" }}
        >
          <h2 style={h2}>一句话定位</h2>
          <p style={p}>
            <strong>Context OS</strong>
            ：把个人/小团队知识收成「可检索、可分享、可被外接 Agent 调用」的工作区。  
            <strong>笔记内置 AI / 第二大脑类应用</strong>
            ：写作与整理体验优先，AI 服务文档工作流。  
            <strong>通用 RAG 套件 / 自建栈</strong>
            ：最大灵活度，工程与运维成本也最高。
          </p>

          <h2 style={h2}>能力对照（类别级，非单品跑分）</h2>
          <div style={{ overflowX: "auto", margin: "0.75rem 0" }}>
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <thead>
                <tr>
                  <th style={th}>维度</th>
                  <th style={th}>Context OS</th>
                  <th style={th}>笔记 AI / 第二大脑类</th>
                  <th style={th}>通用 RAG / 自建</th>
                </tr>
              </thead>
              <tbody>
                {ROWS.map((row) => (
                  <tr key={row.dim}>
                    <td style={td}>{row.dim}</td>
                    <td style={td}>{row.cos}</td>
                    <td style={td}>{row.notes}</td>
                    <td style={td}>{row.rag}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <h2 style={h2}>更适合选 Context OS 的情况</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            <li>知识要以「工作区」为单位隔离，并可能对访客开放问答。</li>
            <li>希望 Cursor / Claude 等外接 Agent 通过 MCP 读同一库，而不是只在笔记 UI 内聊。</li>
            <li>接受「会员管分享名额、余额管模型」的拆分，而不是只要一个写笔记席位。</li>
            <li>需要公开可复制的 Agent Pack 与工作区密钥边界（见 Agent 文档）。</li>
          </ul>

          <h2 style={h2}>可能更适合其它方案的情况</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            <li>日常主战场是长文写作、块编辑与团队 wiki，且 AI 仅作写作辅助。</li>
            <li>必须深度定制检索、重排、权限与多租户计费，并有工程团队维护。</li>
            <li>不需要对外分享或 Agent 接入，只想在单一笔记应用内完结。</li>
          </ul>

          <h2 style={h2}>不做什么声明</h2>
          <p style={p}>
            本页不声称 Context OS「全面优于」任一具体竞品，也不给出未核验的市场份额、延迟或准确率数字。竞品能力以各厂商当前文档为准；若你评估具体产品，请以其官方说明为证据。
          </p>

          <h2 style={h2}>下一步</h2>
          <ul style={{ lineHeight: 1.7, margin: "0.35rem 0 0.65rem", paddingLeft: "1.25rem" }}>
            <li>
              产品事实问答：<Link href="/help/faq">FAQ</Link>
            </li>
            <li>
              外接 Agent：
              <Link href="/help/api-access/agents">Agent API</Link> ·{" "}
              <Link href="/help/api-access">人类接入说明</Link>
            </li>
            <li>
              名额与余额：<Link href="/pricing">定价</Link>
            </li>
            <li>
              进入产品：<Link href="/">应用入口</Link> · <Link href="/register">注册</Link>
            </li>
          </ul>
        </article>
      </div>
    </main>
  );
}
