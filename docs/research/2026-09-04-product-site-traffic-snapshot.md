# Context OS 产品站流量快照（只读聚合）

**观察窗口**：2026-08-21 00:00 至 2026-09-04 00:00（Asia/Shanghai，14 个完整自然日）  
**生成日期**：2026-09-04  
**数据源**：生产主机 Nginx access logs；生产 PostgreSQL `product_events` / `daily_product_metrics` 只读聚合；公开 URL 健康检查  
**隐私边界**：未导出或保留原始 IP、邮箱、用户 ID、请求体、凭据或其它个人信息；本文只有聚合数字。未修改生产配置、数据或代码。

## 1. 结论

- 站点不是完全没有访问，但能够进入产品动作层的量很小。
- 14 天内识别到 24 次带 `contextlm_portal` UTM 的 portal → app 页面访问，其中 11 次到登录页、13 次到客户端下载页。
- 同期产品事件里有 13 个注册用户；按邮箱域名规则识别出 10 个明显的内部/测试候选，剩余 3 个只能称为“公共邮箱代理样本”，不是已核实的外部真人。其中 1 个完成了有引用回答的激活代理事件。
- 当前无法把上述注册或激活归因到具体 UTM：入口参数没有贯穿登录/注册并写入产品事件；`daily_product_metrics` 也没有任何行。
- 标准搜索爬虫能够访问公开页面，因此“robots 或全站不可抓取”不是现有证据支持的主解释；但没有 GSC/Bing 导出，仍不能确认实际收录、曝光、查询或 CTR。

## 2. 访问量：14 日总览

下表经过启发式清洗，但仍是**上限估计**。`visitor-day` 是“同一网络地址在同一天、同一 surface 至少有一次合格页面访问”，不是 cookie 用户或真人会话；各 surface 会重复计入同一个访客，因此不能相加当作全站人数。

| Surface | Visitor-days | 去重路径访问 | 原始页面访问 | 窗口内唯一网络地址 | 解释 |
|---|---:|---:|---:|---:|---|
| Portal | 554 | 554 | 586 | 421 | 品牌门户；页面结构单一 |
| Blog | 1,269 | 1,328 | 2,045 | 835 | 量最大，但历史内容中大量文章与当前产品意图弱相关 |
| App public | 358 | 399 | 554 | 290 | 公开产品、帮助、集成等页面 |
| App auth | 24 | 28 | 35 | 23 | 登录、注册、找回密码 |
| App product | 12 | 12 | 18 | 11 | 需要更深产品意图的页面 |

### 前 7 天与后 7 天

| Surface | 2026-08-21 至 08-27 visitor-days | 2026-08-28 至 09-03 visitor-days | 变化 |
|---|---:|---:|---:|
| Portal | 263 | 291 | +10.6% |
| Blog | 625 | 644 | +3.0% |
| App public | 209 | 149 | -28.7% |
| App auth | 12 | 12 | 0.0% |
| App product | 8 | 4 | -50.0% |

这些变化只说明日志中的访问轮廓：样本小、身份口径弱且仍可能含分布式自动化，不能据此宣称增长或流失。真正值得关注的是 app public 与 app product 没有随 portal 同向增长。

## 3. 能够观察到的下游动作

### Portal UTM 到站

| 来源/位置 | 目标 | 页面访问 |
|---|---|---:|
| `contextlm_portal` / nav | Login | 5 |
| `contextlm_portal` / product section | Login | 4 |
| `contextlm_portal` / footer | Login | 2 |
| `contextlm_portal` / product section | Desktop | 7 |
| `contextlm_portal` / nav | Desktop | 4 |
| `contextlm_portal` / footer | Desktop | 2 |
| **合计** |  | **24** |

窗口内没有观察到其它结构完整、可解释的营销 campaign UTM 到站。由于登录页只读取 `next`、注册页只读取 referral/next，且注册产品事件没有存营销来源，24 次到站不能与注册或激活做用户级连接。

### 产品事件代理样本

| 指标 | 聚合值 | 证据边界 |
|---|---:|---|
| 注册用户 | 13 | 事件期内全部注册；含明显测试/内部候选 |
| 明显内部/测试候选 | 10 | 依据 `example` / `test` / `contextlm` 等域名模式；其中 10 个集中在 08-30 21:00 左右 |
| 公共邮箱代理注册 | 3 | 仅排除明显测试域名，不代表已核实外部真人 |
| 上述代理样本创建 workspace | 3 | 产品事件聚合 |
| 上述代理样本完成资料上传 | 1 | 产品事件聚合 |
| 上述代理样本产生有引用回答 | 1 | 以 `chat_completed` 且 `citation_count > 0` 为代理定义 |
| 上述代理样本打开引用 | 1 | 产品事件聚合 |
| `daily_product_metrics` 行数 | 0 | 当前没有可用的每日漏斗汇总 |

补充观察：窗口内有 21 次登录事件，涉及 13 个用户；14 次 `chat_completed` 全来自同一用户，其中 9 次引用数为 0、5 次引用数大于 0。全表共 1,462 条 `product_events`，时间跨度为 2026-07-14 至 2026-09-01；09-01 之后未见新产品事件，这可能表示没有动作，也可能表示事件采集缺口，不能单独定性。

## 4. 搜索与抓取信号

- `https://contextlm.top/`、portal robots/sitemap、app 首页/robots/sitemap、公开 integrations 页面在检查时返回 200。
- App sitemap 约有 30 个公开 URL；Portal sitemap 只列根页。
- 14 日日志里 Googlebot 能成功访问 app 48 个去重路径、blog 17 个、portal 14 个；Bingbot 的成功去重路径更少，但也能到达站点。没有识别到 Baiduspider。
- 搜索代理查询 `site:contextlm.top "Context OS"`、`site:app.contextlm.top "AI 知识库"`、`site:blog.contextlm.top "Context-OS"` 与 `"Context OS" "contextlm.top"` 没有返回结果。这只是弱信号，不能替代 GSC/Bing 的实际索引和曝光数据。
- 日志中的外部搜索首触达观测很少：blog Google 9、Baidu 1；portal Google 5、`www` Google 1、DuckDuckGo 2；app Bing 1、DuckDuckGo 1。该口径基于 referrer 日志，不等于平台报告的 session 或 click。

## 5. 清洗与质量限制

- 扫描 63,352 条 Nginx 日志记录，1,012 条未被解析。
- 识别并排除已知 crawler/scanner、静态资源、API/health、纯错误访问，以及异常高频 visitor-day：每日超过 100 请求、超过 20 个内容页面访问或超过 40 个错误响应。
- 109 个可疑 visitor-days 与 1,683 个可疑页面访问被排除；最终启发式集合包含 2,204 个全站 visitor-days。跨 surface 统计不是互斥集合。
- 无 cookie/session id、浏览器指纹或用户同意后的客户端分析，因此无法可靠地区分共享网络、多设备、预取和浏览器外观的自动化。
- Nginx 日志通常高估真人访问；客户端 Analytics 可能因同意、脚本阻止或页面未加载而低估。两者应互相校验，不应相加。
- 当前没有 Search Console、Bing Webmaster、GA4 或广告平台的原始导出，因此无法回答实际 impressions、indexed pages、CTR、organic sessions、paid spend 或 CAC。

## 6. 可复核的实现断点

- `frontend_next/app/(auth)/login/page.tsx` 只读取 `next`，从登录到注册的链接没有保留 UTM。
- `frontend_next/app/(auth)/register/page.tsx` 只读取 referral/next，注册 payload 不含营销来源。
- `avrag-rs/crates/transport-http/src/lib_impl/auth_primary.rs` 的 `UserRegistered` metadata 只记录 `email_domain`。
- `ANALYTICS_ROLLUP_ENABLED=false`；`daily_product_metrics` 为 0 行。
- `avrag-rs/bins/worker/src/analytics_jobs.rs` 的旧汇总仍检查 `notebook_created`，而当前事件是 `workspace_created`；激活判断要求多个动作发生在同一自然日，且没有用 `citation_count` 定义“有引用回答”。即使直接开启，也会得到与营销口径不一致的激活结果。

## 7. 当前最稳妥的解释

现有证据不支持“技术完全不可抓取”或“只需要多发文章”。更完整的解释是四个漏点叠加：

1. **测量漏点**：UTM 在认证链路丢失，产品事件不能回溯渠道，日汇总关闭且逻辑过时。
2. **需求与执行漏点**：已有关键词计划没有变成稳定发布、分发和复盘闭环；实时 GSC 需求证据仍缺失。
3. **转化漏点**：Portal 首屏以品牌叙事和创作者/产品双入口为中心，没有把一个明确产品任务、可见输出和主 CTA 放在首屏形成单一重力。
4. **方案漏点**：早期 PoC、Portal 转化、SEO/GEO、内容军火库分散成多套计划；部分状态写“完成”，但实际基线和真实业务指标仍为空。

本文是 2026-09-04 的一次性快照，不是持续监控报表。
