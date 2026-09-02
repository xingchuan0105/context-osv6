# Context OS 英文公开面方案（English public surface）

**日期**: 2026-09-01 · **状态**: Active · **上游**: `docs/plans/2026-08-11-contextlm-geo-seo-optimization-plan.md` §8 国际化
**范围**: `app.contextlm.top` 公开 SEO 面（首页 / pricing / desktop / help 族 / legal 族）
**非目标**: 登录后 App（dashboard/settings）保持 cookie 双语，不 URL 化

---

## 1. URL 策略（已确认：A）

- zh 页：`/help/faq` 等（现状不变）
- en 页：`/en/help/faq` 等（`/en/` 静态段 + 薄包装）
- 不引入 next-intl 全站 localePrefix（避免破坏登录后路由与桌面端静态导出）

## 2. 实现方式

- 内容抽成单一数据源：`frontend_next/lib/content/faq.ts`、`lib/content/compare.ts`（zh/en 两份）
- 共享页面组件：`frontend_next/components/help/faq-page.tsx`、`compare-page.tsx`（按 `locale` 渲染）
- 路由薄包装：`app/(open)/help/faq/page.tsx`（zh）与 `app/en/help/faq/page.tsx`（en）各自定义 metadata + hreflang
- JSON-LD 同源：`FaqPageJsonLd` 按 locale 读 `faqContent`，`inLanguage` 随 locale

## 3. 内容硬约束（HCU）

- 人工翻译，不机翻；产品名 Context OS / 品牌 ContextLM 不译
- 英文页同样带：作者明牌（Xing Chuan + 链接）、更新日期、证据来源、结构化数据
- 术语：可分享名额 share slots · 工作区 workspace · 余额充值 wallet top-up · BYOK/MCP/RAG 保留缩写

## 4. 切片

| Slice | 内容 | 状态 |
|---|---|---|
| 1 | `/en/help/faq` + `/en/help/compare` + hreflang 双向 + sitemap + llms.txt | ✅ 完成 |
| 2 | `/en` 首页 SSR 摘要 + `/en/pricing` + `/en/desktop` | ✅ 完成 |
| 3 | `/en/help/api-access*` + `/en/legal/*` | ✅ 完成（terms/privacy 除外，见 §6） |
| 4 | 测量：D2 英文种子问 + diagnose 复跑 | ✅ 完成（D2 人工抽检待录，见主方案 §19） |

## 5. 验收

- build 后 curl `/en/help/faq`：en 正文 + `hreflang` 双向 + `FAQPage` `inLanguage=en`
- `tests/seo/public-seo.test.ts` 增补 en 页 canonical / sitemap 断言

## 6. 法律文档例外（terms / privacy）

`/en/legal/terms` 与 `/en/legal/privacy` 暂不生成英文版：法律文本需人工法律审校，
不机翻。当前 zh 页保持 `zh-CN + x-default`（无 en hreflang），英文法律中心卡片
仍链接到 zh 原文。翻译交接单（含术语表、frontmatter 模板、双源文、自检清单）：
`docs/plans/2026-09-01-legal-en-translation-handoff.md`。译文交回后按 §4 同一模式
落盘 `content/legal/en/*.mdx` + 两个 en 路由。
