/**
 * llms.txt（GEO 方案 A4）：agent-readable 站点索引。
 * public/ 在 standalone 部署不对外（middleware-routing.ts 里 /docs/*.md 同款问题），
 * 故走 app route。
 */
export const dynamic = "force-static";

const LLMS_TXT = `# Context OS

> 可分享的个人知识工作区：文档入库与问答、通过 MCP / API 外接 Agent、会员解锁可分享名额。

## Public docs

- Agent API 接入（agent-readable）: https://app.contextlm.top/help/api-access/agents
- 人类 API 接入说明: https://app.contextlm.top/help/api-access
- 定价与充值: https://app.contextlm.top/pricing
- 桌面客户端（免费）: https://app.contextlm.top/desktop
- 品牌官网: https://www.contextlm.top
`;

export function GET() {
  return new Response(LLMS_TXT, {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
