# 百度搜索资源平台接入清单（Context LM / Context OS）

**日期**: 2026-08-12  
**状态**: 操作手册（需人工在百度账号完成；本仓库不代替登录）  
**官方入口**: https://ziyuan.baidu.com/  
**关联**: GEO 方案 · GSC 清单 `2026-08-12-gsc-onboarding-checklist.md`  

**非目标**：保证收录/排名；改生产后端；代替 ICP 备案。

---

## 0. 和 GSC 的差别（先建立预期）

| | Google Search Console | 百度搜索资源平台（旧称「站长平台」） |
|--|------------------------|--------------------------------------|
| 入口 | search.google.com/search-console | **ziyuan.baidu.com** |
| 验证 | DNS TXT / HTML / 文件 | **文件 / HTML 标签 / CNAME**（无 Google 那种 Domain 一网打尽 TXT） |
| Sitemap | 属性内提交 XML | 「普通收录 → sitemap」贴 URL |
| 海外站 | 正常 | 服务器在海外 + Cloudflare 时，**抓取往往更慢、收录更少** |
| 备案 | 不需要 | 面向中国大陆用户时，**未备案常影响展示**（本清单不处理备案） |

工程侧已具备：公开页 SSR、sitemap 200、`robots.txt` 已显式 `Allow` **Baiduspider** / **Baiduspider-render**。

---

## 1. 准备

1. 一个 **百度账号**（常用手机号注册即可）：https://passport.baidu.com/  
2. 登录 https://ziyuan.baidu.com/  
3. 能改 **Cloudflare DNS**（推荐 CNAME 验证），或能往站点根目录丢一个 html 文件（文件验证）。

建议添加的站点（**分别添加、分别验证**）：

| 站点 URL（必须带 https） | 角色 |
|--------------------------|------|
| `https://contextlm.top` | 品牌 marketing（首选 apex） |
| `https://app.contextlm.top` | 产品公开面（定价 / FAQ / 对比 / Agent） |

`www.contextlm.top` 可选。HTML canonical 已指向 apex；不优先加 www，避免两套数据。

---

## 2. 添加站点

1. 打开 https://ziyuan.baidu.com/site/index （用户中心 → **站点管理**）。  
2. **添加网站**。  
3. 填写完整 URL：先加 `https://contextlm.top`（协议选 **https**，不要漏写）。  
4. 站点领域：选接近的（如「软件 / IT / 互联网」或「人工智能」——以页面当前分类为准，选错可后改）。  
5. 进入验证。

---

## 3. 验证方式（三选一）

### 方案 A — CNAME（推荐，与 GSC 一样不动代码）

1. 验证方式选 **CNAME**。  
2. 百度会给一条类似：  
   - 主机记录：`xxx.verify.baidu.com` 或一串随机子域  
   - 记录值：百度指定的目标  
3. Cloudflare → `contextlm.top` → DNS → **Add record**：  
   - Type: `CNAME`  
   - Name / Target：按百度页面原样填  
   - Proxy：**DNS only（灰云）**，不要橙云代理  
4. 等 1～30 分钟后，回百度点 **完成验证**。  

`app.contextlm.top` 再做一遍（可能是另一条 CNAME，或验证该子域）。

### 方案 B — 文件验证（marketing 最简单）

1. 下载百度给的 `baidu_verify_*.html`。  
2. 放到对应站**网站根目录**，使以下 URL 能 200：  
   - marketing：`https://contextlm.top/baidu_verify_xxxxx.html`  
   - app：`https://app.contextlm.top/baidu_verify_xxxxx.html`  
3. marketing：文件丢进 `~/context-os-landing/public/` 后部署 landing。  
4. app：文件丢进 `frontend_next/public/` 后部署 frontend（Next 会按路径提供静态文件）。  
5. 浏览器能打开该 html 后，回百度点 **完成验证**。  
6. **验证成功后不要删文件**（删了可能掉验证）。

此方案要发版；token 不要写进聊天记录长期传播即可，文件本身进仓库无密钥风险。

### 方案 C — HTML 标签

百度给一段 `<meta name="baidu-site-verification" content="……">`。  
- marketing：加到 layout 的 `<head>` 后部署。  
- app：加到 `frontend_next/app/layout.tsx` metadata 后部署。  

不如 CNAME 干净，适合暂时不能改 DNS、也不能丢文件时。

**建议：能改 Cloudflare 就走方案 A。**

---

## 4. 提交 Sitemap

验证成功后：

**数据引入 / 普通收录 → sitemap**（界面文案可能是「链接提交 → 自动提交 → sitemap」）。

分别粘贴（每个站点在**自己的属性**里提交自己的图）：

```
https://contextlm.top/sitemap.xml
https://app.contextlm.top/sitemap.xml
```

说明：

- 必须是 **xml**，公网 200。  
- 百度对 **sitemap 索引文件**支持差；我们现在是单文件 urlset，符合。  
- 提交只加快「发现」，**不保证收录**。  
- **不要**提交 `/dashboard`、`/settings`、`/shared/*`。

可选：同一页的「手动提交」把下面几条再丢一次（加速发现，非必须）：

```
https://contextlm.top/
https://app.contextlm.top/
https://app.contextlm.top/pricing
https://app.contextlm.top/help/faq
https://app.contextlm.top/help/compare
https://app.contextlm.top/help/api-access
https://app.contextlm.top/help/api-access/agents
```

---

## 5. 验证后看什么

| 报告 | 含义 |
|------|------|
| **抓取诊断 / 抓取异常** | 百度蜘蛛能否打开；海外+CF 常见超时或空抓 |
| **索引量** | 已收录条数；新站可能长期为 0 |
| **sitemap 状态** | 是否抓到地图里的 URL |
| **流量与关键词** | 有展示后才有；可能很久没有 |

在百度搜：`site:contextlm.top` 或 `site:app.contextlm.top` 可粗看收录（不精确）。

---

## 6. 海外站常见坑（提前知道）

1. **服务器不在国内**：百度蜘蛛不稳定，收录慢或极少——属预期，不是 robots 写错。  
2. **Cloudflare**：若仍对部分 UA 挑战/拦截，Baiduspider 可能抓失败。AI Crawl 已关；若抓取诊断失败，在 CF 对 `Baiduspider` 放行或降低安全级别。  
3. **未 ICP 备案**：国内结果页展示受限；备案是另一条合规线，本清单不做。  
4. **apex / www 双 200**：只验证并主推 `https://contextlm.top`。  
5. **提交 API 推送**：需要 token + 服务端调用，对新站收益有限，**本轮不做**。

---

## 7. 完成后勾选

- [x] 百度账号可登录 ziyuan.baidu.com  
- [ ] `https://contextlm.top` 验证成功（文件已上线，待控制台点「完成验证」）  
- [ ] `https://app.contextlm.top` 验证成功（文件已上线，待控制台点「完成验证」）  
- [ ] 两条 sitemap 已提交  
- [ ] （可选）抓取诊断对首页「抓取成功」  

**marketing 验证文件（2026-08-12）**：已部署  
`https://contextlm.top/baidu_verify_codeva-FzCesrSAvK.html` → **200**，正文与桌面原件一致。成功后勿删 `context-os-landing/public/` 下该文件。

验证 token / CNAME 主机名 **不要**贴进 git。做完可以说一声，方案里可记「百度站长已接」。
