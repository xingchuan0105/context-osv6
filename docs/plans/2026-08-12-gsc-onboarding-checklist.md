# Google Search Console 接入清单（Context LM / Context OS）

**日期**: 2026-08-12  
**状态**: 操作手册（需人工在 GSC 控制台完成；本仓库不代替 Google 账号操作）  
**关联**: GEO/SEO 方案 D1 · `docs/plans/2026-08-11-contextlm-geo-seo-optimization-plan.md` §16–§17  

**非目标**：提高排名保证、改生产代码、代你登录 Google。

---

## 0. 上线前已具备（工程侧）

| 项 | 现状（2026-08-12 探测） |
|----|-------------------------|
| App 公开 sitemap | `https://app.contextlm.top/sitemap.xml` → **200**（含 `/`、`/pricing`、`/help/faq`、`/help/compare` 等） |
| Marketing sitemap | `https://contextlm.top/sitemap.xml` → **200**；`www` 同源文件亦 **200** |
| App robots | `https://app.contextlm.top/robots.txt` → Allow 公开 + Disallow 私有路径 + Sitemap 行 |
| Marketing robots | `https://contextlm.top/robots.txt` → Allow + AI bot Allow + 双域 Sitemap 行 |
| Marketing canonical | apex 与 www 页面均指向 **`https://contextlm.top`**（首选 apex） |
| App 公开页 canonical | 自引用（如 `/help/faq` → `https://app.contextlm.top/help/faq`） |
| CF AI bot robots 块 | 用户确认已关 |

**注意**：`contextlm.top` 与 `www.contextlm.top` 目前均 **HTTP 200**，未强制 301 到单一 host。HTML canonical 已选 apex。GSC 建议验证两个 host（或 Domain property 一次覆盖），但 **以 apex 为对外首选** 提交 sitemap、看索引。

---

## 1. 准备材料

1. 可登录 [Google Search Console](https://search.google.com/search-console) 的 Google 账号（建议用主理人/公司主账号，勿用个人临时号）。  
2. 对 `contextlm.top` DNS 的编辑权（Cloudflare 控制台即可）——**推荐 DNS TXT 验证**。  
3. （可选）对 `app.contextlm.top` 同一 zone 的 DNS 权——Domain property 一次覆盖全部子域。  

---

## 2. 推荐属性形态（二选一）

### 方案 A — Domain property（推荐）

- 添加属性类型：**网域（Domain）**  
- 输入：`contextlm.top`（不要加 `https://`）  
- 验证方式：Google 给出的 **DNS TXT** 记录写到 Cloudflare（名称通常为 `@` 或 Google 指定主机名）  
- **一次验证**覆盖 `contextlm.top`、`www.`、`app.`、`blog.` 等所有子域  

优点：以后少加属性。  
缺点：必须能改 DNS；CF 代理开启时 TXT 仍写在 DNS 即可（TXT 不依赖代理橙云）。

### 方案 B — 网址前缀（URL-prefix）×2～3

若暂时不能 Domain 验证，分别添加：

| 属性 URL | 用途 |
|----------|------|
| `https://app.contextlm.top/` | SaaS 公开面（定价、帮助、FAQ、对比） |
| `https://contextlm.top/` | 品牌 marketing（首选） |
| `https://www.contextlm.top/` | 可选；防止 www 被单独索引却不在报表里 |

验证方式任选其一（优先级建议）：

1. **DNS TXT**（与 Domain 类似，但可按子域）  
2. **HTML 标签**（需改 marketing/app 的 `<head>`——要发版，不如 DNS）  
3. **HTML 文件**（丢到站点根——static 站可行，app standalone 更烦）  

**建议：能改 Cloudflare 就走方案 A。**

---

## 3. Cloudflare DNS 验证步骤（方案 A）

1. GSC → 添加资源 → **网域** → `contextlm.top`。  
2. 复制 Google 提供的 TXT 值（形如 `google-site-verification=……`）。  
3. Cloudflare → 该域名 → **DNS** → **Add record**：  
   - Type: `TXT`  
   - Name: `@`（或 Google 写明的主机名）  
   - Content: 整段 verification 字符串  
   - Proxy: DNS only（TXT 无代理概念，保持默认即可）  
4. 保存后等 1～30 分钟（偶发更久）。  
5. 回到 GSC 点 **验证**。  
6. 成功后书签该属性首页。

自检（本机）：

```bash
dig TXT contextlm.top +short
# 或
dig TXT contextlm.top @1.1.1.1 +short
```

应看到含 `google-site-verification` 的记录。

---

## 4. 提交 Sitemap

在**已验证**的属性中：

左侧 **索引编制 → Sitemaps**（或「sitemap」搜索），分别添加：

| Sitemap URL | 期望 |
|-------------|------|
| `https://app.contextlm.top/sitemap.xml` | 状态「成功」；已发现 URL 数 ≥ 当前公开列表（约 13 条，随发版变） |
| `https://contextlm.top/sitemap.xml` | 状态「成功」；至少含 marketing 首页 |

**不要**提交：

- `/dashboard/*`、`/settings`、`/shared/*`（已不在 sitemap；robots 亦 disallow）  
- 仅内网或鉴权后才有正文的 URL  

提交后 24～72 小时再看「已编入索引」；首次可能长时间「已发现 - 尚未编入索引」，属正常。

---

## 5. 建议立刻做的 URL 抽检（GSC「网址检查」）

对下列 URL 各执行一次 **检查完整网址 / 请求编入索引**（勿对整站 bulk 狂点）：

**App（优先 GEO 内容）**

1. `https://app.contextlm.top/`  
2. `https://app.contextlm.top/pricing`  
3. `https://app.contextlm.top/help/api-access`  
4. `https://app.contextlm.top/help/api-access/agents`  
5. `https://app.contextlm.top/help/faq`  
6. `https://app.contextlm.top/help/compare`  
7. `https://app.contextlm.top/desktop`  

**Marketing**

8. `https://contextlm.top/`  

验收勾选：

- [ ] 「网址是否在 Google 上」：可抓取  
- [ ] 用户声明的 canonical 与 Google 选择的 canonical **一致或合理**  
- [ ] 无 `noindex` / 软 404  
- [ ] 抓取到的 HTML 含可见 H1（非仅「加载中…」）

---

## 6. 日常看什么（D1 验收）

| 报告 | 何时算「已接入」 |
|------|------------------|
| **Sitemaps** | 两条 sitemap「成功」，无长期「无法获取」 |
| **网页索引编制** | 有「已编入索引」计数；严重错误趋近 0 |
| **效果**（搜索） | 2～4 周后有展示/点击数据（新站可能很慢） |
| **体验**（Core Web Vitals 等） | 可选，非本轮 P0 |

**禁止**：把 GSC 展示量/点击量写进对外营销承诺，或写进 GEOHub 分数对比当「引用率」。

---

## 7. 已知结构问题（非 GSC 按钮能修）

| 问题 | 影响 | 建议 |
|------|------|------|
| apex 与 www 双 200、无 301 | 可能稀释抓取；GSC 可能看到两套 URL | 中期在 CF 把 `www` → `https://contextlm.top` 301；与现有 HTML canonical 对齐 |
| `site-map.ts` 默认 hub 为 `www` | 产品内链可能指向 www | 与 canonical apex 统一（另开工程项，非本清单必做） |
| 登录后路由 | 不应进索引 | 已 robots disallow + 不进 sitemap；若 GSC 出现可「移除」或保持 disallow |

---

## 8. （可选）Bing Webmaster

1. https://www.bing.com/webmasters  
2. 可导入 GSC 属性，或 DNS/文件验证 `contextlm.top`  
3. 提交同一组 sitemap URL  

非必须；做完 GSC 即可勾 D1 主验收。

---

## 9. D2 预告：月度 AI 引用抽检（与 GSC 独立）

GSC 管 **Google 网页搜索**；AI 引用需另记。建议每月固定 3 问（中文），在 ChatGPT / Perplexity 各跑一次，记录是否出现品牌/URL：

| # | 种子问（示例） |
|---|----------------|
| 1 | 有没有适合 Cursor / Claude 用 MCP 接的个人知识库产品？ |
| 2 | Context OS 和笔记自带 AI、自建 RAG 有什么区别？ |
| 3 | 可分享工作区会员和模型余额分别是干什么的？ |

结果可存 `docs/plans/geo-seo-briefs/measure-YYYY-MM.json`（字段自定：平台、问句、是否提及、引用 URL、日期），再按需丢给 GEOHub `geo-measure`。

---

## 10. 完成后回写

操作人在本文件底部或方案 §17 勾选：

- [x] Domain 或 URL-prefix 属性已验证  
- [x] App sitemap 已提交且「成功」  
- [x] Marketing sitemap 已提交且「成功」  
- [x] 至少 3 个优先 URL 做过「网址检查」  
- [ ] （可选）www→apex 301 已排期  

**完成记录**：2026-08-12 用户确认 GSC 接入完成（验证 + sitemap 提交）。无需将 verification token 写入 git。

验证完成后把 **属性类型**（Domain / Prefix）与 **提交日期** 记一行即可，无需贴 verification token 进 git。
