# E4.4 任务记录:`/en/*` 英文公共站(13 端点,E4 收官)

| 字段 | 内容 |
|---|---|
| 日期 | 2026-09-05 |
| 负责人 | Agent / Solo Trunk |
| 关联计划 | [`2026-09-05-development-execution-plan.md`](2026-09-05-development-execution-plan.md) §6 E4 |
| 门禁目标 | **E4.4 切片完成,E4 全部 71/71 端点挂载完毕** |

---

## 1. 任务目标与交付范围

按矩阵 §11 挂载英文公共站 13 端点,内容逐字对齐 Next en 数据源:

1. **`/en`**:复用 `ProductHomePage(locale="en")`(E4.3 已双语化),en 价值主张 + Organization/SoftwareApplication JSON-LD `inLanguage: en`。
2. **`/en/pricing`**:en SEO 头 + 复用 zh 定价组件(套餐数据同源后端契约)。**已知偏差**:定价页 UI 文案暂为 zh(28 处字符串),en 文案待后续切片;canonical/hreflang/数据对齐。
3. **`/en/desktop`**:`DesktopProductPage(locale="en")`(E4.2 已双语化),en 全文案。
4. **`/en/help/faq|compare`**:FAQ_EN / COMPARE_EN 内容常量(逐字对齐 `lib/content/*.ts` en),页面 locale 参数化;FAQPage JSON-LD `inLanguage: en`。
5. **`/en/help/api-access`**:API_ACCESS_EN(i18n `helpApiAccess*` en 值);**`/en/help/api-access/agents`**:正文本为英文共享,仅 SEO 头切换。
6. **`/en/legal` ×6**:法务组件全部 locale 参数化(en 文案逐字取自 `legal-center.tsx`/`licenses-summary.tsx` 等 en 值;`content/legal/en/{terms,privacy}.md` 编译期内嵌);LegalShell 页脚 en 化("Last updated"/"Contents"/"Back to legal center")。
7. **SEO 对齐**:en 页 canonical=/en/…,hreflang zh-CN ↔ zh 路径,en ↔ /en/…,x-default 恒指 zh 路径(与 Next 一致);`PublicSeoHead` 重构为显式 `zh_href` 并带 `locale`(JSON-LD 语言随页)。
8. **不变量**:
   - `/en` 与 `/` 同样带会话分流(未登录 → /login)。
   - 无死路径:en CTA/证据链接全部指向已挂载路由。
   - 不留兼容层:zh 页面改为 locale 参数化组件 + 包装器,无双实现。

## 2. 执行步骤

| 次序 | 工作项 | 状态 | 产物与证据 |
|---|---|---|---|
| E4.4.1 | `public_content.rs`:FAQ_EN / COMPARE_EN / API_ACCESS_EN + copy selector 函数 + AUTHOR_LINE_EN | 已完成 | en 内容逐字对齐 |
| E4.4.2 | help 四页 locale 参数化(zh/en 包装器);PublicSeoHead 加 `locale` 与显式 `zh_href` | 已完成 | 全部旧调用点迁移 |
| E4.4.3 | legal_pages locale 参数化(6 页 + Shell + Footer);en md 接入 | 已完成 | en 法务文本逐字一致 |
| E4.4.4 | `EnPricingPage`(en SEO 头 + 复用组件)、EnDesktopPage、EnHomePage | 已完成 | — |
| E4.4.5 | app.rs 13 条 en 路由;routes.rs 13 变体 + parse 测试 | 已完成 | 71 端点全量登记 |
| E4.4.6 | `en-journey.spec.ts` 7 例(无 JS SSR / desktop / pricing / faq hreflang / compare+api-access / legal 三页 / agents canonical) | 已完成 | 全量 Playwright 74/74 |
| E4.4.7 | 矩阵 13 行 + 汇总行更新、图谱更新、本地提交 | 已完成 | 提交号见 git log |

## 3. 验证证据

证据目录 `docs/engineering/_reports/2026-09-05-e4-4/`:

- `cargo test -p web-sdk -p web-ui`:24 套件全部 0 failed
- `cargo check` ssr / wasm hydrate:exit 0、零警告
- `cargo leptos build`:exit 0
- `pnpm exec playwright test`:**74 passed / 0 failed**(67 + 7)

## 4. 剩余问题

- `/en/pricing` UI 文案为 zh(偏差已记录);套餐名等动态数据来自后端契约,后续按需补 en UI 文案。
- robots.txt / sitemap.xml / MarketingShell 营销壳:Next 无对应源文件 / 公开页统一轻壳(既有记录)。
- E0–E4 全部 71 端点挂载完成;后续为 W5 Web 切流与 D0.2–D1 桌面里程碑(GD1)。

## 5. 图谱状态

`code-review-graph update` 已执行。
