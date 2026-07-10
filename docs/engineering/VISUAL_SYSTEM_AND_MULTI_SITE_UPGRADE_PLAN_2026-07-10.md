# Context-OS 统一视觉体系 + 多站点升级计划

**日期**: 2026-07-10  
**状态**: Draft for execution  
**基准产品**: `context-osv6/frontend_next`（结构与 token 深度）  
**色彩方向**: **黑白灰 + 单一标志色**（弃用青 / 绿；对齐 Grok / OpenAI / Claude 类「克制中性」产品）  
**样式源规范（Canonical）**: [`docs/design/STYLE_BASELINE.md`](../design/STYLE_BASELINE.md) — 色板 / 字阶 / 控件契约以该文件为准  
**Ingestion 卡住交接（并行）**: [`docs/engineering/INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md`](INGESTION_PDF_STUCK_DIAGNOSIS_2026-07-10.md) — PDF worker 超时，**不阻塞**本 UI 计划  
**范围**: App · Landing · Blog(Ghost) · Why I Am Right · 象棋(canju) · Workspace 连通性 · **Next 产品 UX（§6a U1–U14）**  

---

## 0. 一句话结论

| 议题 | 结论 |
|------|------|
| 视觉以谁为基准 | **v6 的结构 / 字阶 / 圆角 / 组件纪律**；**颜色整盘换成中性墨色系** |
| 青 / 绿 | **全部退场**（含 landing/blog/why 的 `#10b981` 与 v6 的 cyan accent） |
| 多站点 | 统一 **Cos Shell**（logo / nav / footer / tokens）；场景可保留皮肤 |
| Workspace「没连上」 | **不是纯前端 bug**：生产仍跑 **context-os-v3 Docker**，`/api/` → 3002 旧后端；**v6 Rust (avrag-rs) 未接管 app 域名**；本机 8080 也常未起后端 |

---

## 1. 站点与路径清单

| 产品 | 本地路径 | 线上域名 | 当前运行时（VPS 实测 2026-07-10） |
|------|----------|----------|----------------------------------|
| **Context OS App** | `/home/chuan/context-osv6/frontend_next` + `avrag-rs` | `app.contextlm.top` | Nginx → **前端 3003 / API 3002（v3 Docker）**；主机另有 next@3000，**未作为 app 入口** |
| **品牌 Landing** | `/home/chuan/context-os-landing` | `contextlm.top` / `www` | 静态/Next 落地页，暗色 + 翠绿 |
| **Ghost Blog** | 主题源：`/home/chuan/context-os-theme`（无完整 Ghost 安装） | `blog.contextlm.top` | Docker Ghost `:2368` |
| **Why I Am Right** | `/home/chuan/whyiamright`（目录拼写 *whyiam*） | `whyimright.contextlm.top` | 独立前后端 + 翠绿顶栏 |
| **象棋残局** | `/home/chuan/cchess` | `canju.contextlm.top` | `cchess` → **:8080**；与 App API 端口易混淆 |

---

## 2. 合并审查：视觉现状

### 2.1 三套「宇宙」（当前）

```text
A. 公域 Cos Dark          B. 产品 Precision Lab (v6)     C. 象棋 Canju
   #0a0a0a + #10b981 绿      亮色 + 青 accent               宣纸色 + 棋红/蓝
   Geist / Inter             Space Grotesk + IBM Plex      系统字体
   绿块三线 logo             ContextOsMark 双弧             无品牌壳
```

用户从 Landing「应用」进入 App，会感到 **换品牌**；象棋完全游离。

### 2.2 v6 前端（已有基础）

**优点**

- 有完整 `design-tokens.css` + 字阶 / 间距 / 圆角 / 阴影
- Dashboard / Workspace shell 大体走 token
- 有动效意图（cardEnter / messageEnter 等）
- Precision Lab 文档方向清晰（见 `frontend_next/docs/superpowers/specs/2026-04-27-visual-overhaul-design.md`）

**问题（审美 + 工程）**

| 优先级 | 问题 |
|--------|------|
| P0 | **主色是青**；CTA 默认近黑、hover 才青 → 品牌记忆弱且你不喜欢青 |
| P0 | **中文场景 Display 字体失效**（Space Grotesk 仅 latin） |
| P1 | `dashboard-*` / `workspace-*` / 全局 token **三套平行**，阴影大量硬编码 `rgba(15,23,42,…)` 暗色翻车 |
| P1 | Settings / Admin / API Access **inline style 堆场**，副屏像草稿 |
| P1 | 空态 / 加载多为纯文案，无 skeleton |
| P2 | Logo mark 硬编码 `#0F1117`；OG 仍写 Inter |
| P2 | mono 不统一（JetBrains vs system mono） |
| P2 | 输入框 `translateY` 微动偏抖，不像「精密仪器」 |

### 2.3 公域三站（Landing / Blog / Why）

**优点**

- 暗色 surface / border / text 透明度阶梯已对齐
- Why 已有 `UnifiedNavbar` 互链
- Ghost `brand.css` 与 landing 同源思路

**问题**

- 标志色 **翠绿** 需废除
- Logo **绿块三线** ≠ App **ContextOsMark**
- 字体 Geist vs Ghost Inter 分裂
- token 在 3 个仓库各复制一份 hex

### 2.4 象棋

- 棋盘红黑宣纸 **应保留**（领域语义）
- 缺 Cos Shell；圆角全胶囊；与家族无关

---

## 3. 新视觉体系：Monochrome Ink（建议锁定）

> 以 v6 的 **token 结构 / 控件尺度** 为骨架，颜色改为 **OpenAI/Grok 式中性 + Claude 式克制标志色**（但 **不是** Claude 橙照搬，也不是绿/青）。

### 3.1 原则

1. **90% 界面只用黑白灰**（surface / text / border / shadow）
2. **标志色只做点缀**：focus ring、链接、关键选中、品牌 mark 一处高光 — **禁止大面积铺底**
3. **主 CTA = 墨色实心**（近黑 / 近白反相），不是彩色块
4. **亮色默认**（产品工作台）；暗色为完整第二套，不是反色
5. **公域 Marketing 默认暗底** 也可，但 **同一套 token 名**，仅 `color-scheme` / 默认 theme 不同

### 3.2 色板（HSL 通道写法对齐现 v6）

#### 中性（Light）

| Token | 建议 | 用途 |
|-------|------|------|
| `--background` | `0 0% 100%` | 页底 |
| `--foreground` | `0 0% 9%` | 主文字（近黑，非纯黑刺眼可 `0 0% 12%`） |
| `--surface-muted` | `0 0% 98%` | 侧栏 / 次表面 |
| `--surface-soft` | `0 0% 96%` | hover / 输入底 |
| `--border` | `0 0% 90%` | 标准边 |
| `--muted-foreground` | `0 0% 45%` | 次文 |
| `--subtle-foreground` | `0 0% 60%` | 占位 |

#### 中性（Dark）

| Token | 建议 |
|-------|------|
| `--background` | `0 0% 4%`（`#0a0a0a` 级） |
| `--foreground` | `0 0% 93%` |
| `--surface-muted` | `0 0% 8%` |
| `--border` | `0 0% 16%` |
| `--muted-foreground` | `0 0% 55%` |

#### 标志色（Signature）— **Copper Ink**

弃用：`#10b981` 绿、cyan `#0891b2` / `193 90% 35%`。

| Token | Light | Dark | 用途 |
|-------|-------|------|------|
| `--accent` | `28 55% 42%`（沉铜） | `32 45% 58%`（略亮铜） | 链接、focus、选中描边、少量 chip |
| `--accent-soft` | `30 40% 96%` | `28 20% 14%` | 极淡选中底 |
| `--accent-glow` | 铜 12–18% alpha | 铜 22% alpha | focus ring 外晕 |
| `--cta-background` | `0 0% 9%` | `0 0% 96%` | **主按钮 = 墨/纸，不是铜** |
| `--cta-background-hover` | `0 0% 18%` | `0 0% 86%` | hover 仍中性；可选 **border 带一点 accent** |
| `--ring` / `--focus-ring` | = accent | = accent | 键盘焦点 |

**语义色**（仅状态，不抢标志色）

- success: 中性偏绿灰 **低饱和**（或纯灰 + 图标），避免再变「品牌绿」
- warning / destructive: 保留琥珀 / 玫红，但降低饱和度

> 若你更想 **纯黑白零色相**（更 Grok）：把 `--accent` 也改成 `0 0% 35%` 中灰，仅用字重区分。默认方案仍建议 **Copper**，否则「标志色」不存在。

### 3.3 字体（全站）

| 角色 | 选择 | 说明 |
|------|------|------|
| UI 正文 | **IBM Plex Sans** + 中文系统栈 | 从 v6；比 Geist/Inter 在产品内更稳 |
| 标题 | Space Grotesk **仅拉丁 brand / 数字**；中文标题走 Plex | 避免「假 Display」 |
| 等宽 | **JetBrains Mono** 唯一 | 代码 / FEN / API key |
| Ghost | **去掉 Google Inter**，与上表一致（self-host 或系统栈） |

### 3.4 圆角 / 阴影 / 控件（继承 v6 收紧版）

| 元素 | 值 |
|------|-----|
| 按钮 / 输入 | `8px`（`--radius-control`） |
| 卡片 | `12px` |
| 消息气泡 | `16px` |
| badge / avatar | `999px` only |
| 阴影 | **仅** `var(--shadow-sm|md|lg|xl|focus-ring)`；禁止 `rgba(15,23,42,…)` |

### 3.5 Logo

- **唯一 mark**：`ContextOsMark`（v6 双弧）
- 填色：`currentColor` 或 `hsl(var(--foreground))` / 暗底反白
- **废除** 公域绿块三线 SVG（Landing / Why / Ghost footer）

### 3.6 Cos Shell（全站导航契约）

```text
[Mark] Context-OS
  应用   → app.contextlm.top
  博客   → blog.contextlm.top
  工具   → whyimright · canju（下拉可接受）
Footer: © · 主页 · 应用 · 博客 · 工具
```

- Marketing / Blog / Why / Canju chrome：**固定 56–64px 顶栏**
- App 内：产品 chrome；设置/关于保留家族链接即可

---

## 4. Workspace「没有连接」— 根因与修复

### 4.1 现象（代码侧）

- 前端 `useWorkspaceData` → `GET /api/v1/workspaces/:id` + sessions  
- 失败时 `workspaceLoadError` → 「当前工作区不可用 / 后端不存在」  
- Web 默认：`next.config` rewrite → `API_PROXY_TARGET || NEXT_PUBLIC_API_BASE_URL || http://127.0.0.1:8080`

### 4.2 生产实测（2026-07-10）

| 检查 | 结果 |
|------|------|
| `https://app.contextlm.top` | 200 HTML |
| `https://app.contextlm.top/api/health` | 200 JSON（旧栈） |
| `https://app.contextlm.top/api/v1/workspaces` | **404 HTML**（非 JSON envelope） |
| `https://app.contextlm.top/api/v1/health` | **404** |
| Nginx `app-contextlm.conf` | `/` → `127.0.0.1:3003`；`/api/` → `127.0.0.1:3002` |
| Docker | `context-os-v3-frontend` / `context-os-v3-backend` 等 **v3 栈** |
| 主机 `:8080` | **cchess**，不是 avrag-rs |
| 主机 `:3000` | 另一 Next（`context-os-frontend.service`），**nginx 未把 app 指过去** |
| avrag-rs | **未作为 app 后端监听** |

**结论：用户感觉「workspace 没连上」= 生产 App 仍是 v3 API 面，与 v6 前端契约（`/api/v1/workspaces`…）不对齐；本地若只起前端不起 avrag-rs，同样 100% 失败。**

### 4.3 修复目标

```text
浏览器 → app.contextlm.top
          ├─ /*          → v6 Next (standalone)
          └─ /api/*      → avrag-rs (Rust)   ← 唯一业务 API
cchess 保持 canju.contextlm.top → :8080（象棋专用端口，勿与 App 混用）
```

### 4.4 Workspace 修复任务（P0，可与视觉并行）

| ID | 任务 | 验证 |
|----|------|------|
| W0 | 文档化当前 VPS 拓扑（本计划 §4.2） | 评审通过 |
| W1 | 本机：固定启动 avrag-rs（非 8080 冲突时可用 8081），`API_PROXY_TARGET` 写入 `frontend_next/.env.local` | `curl localhost:$PORT/api/v1/workspaces` 需 401 而非连接失败 |
| W2 | 本机 smoke：登录 → dashboard 列表 → 进 workspace → 发一条消息 | 无「工作区不可用」 |
| W3 | 打包 v6 frontend standalone + avrag-rs release | 产物路径明确 |
| W4 | VPS：部署 avrag-rs 为 systemd（建议 **:8081** 或 unix socket，**避开 cchess :8080**） | `curl 127.0.0.1:8081/health` 与 `/api/v1/...` 契约一致 |
| W5 | Nginx：`app.contextlm.top` 的 `/` → v6 Next；`/api/` → avrag-rs；**SSE/WebSocket 超时与缓冲** | `/api/v1/workspaces` → 401 JSON；前端可登录 |
| W6 | 下线或旁路 v3 Docker（3002/3003）对 app 域名的占用；数据迁移若需单独开单 | app 不再依赖 v3 |
| W7 | 前端 env：`NEXT_PUBLIC_API_BASE_URL` 空 + rewrite；禁止指向 why/cchess | 配置检查清单 |
| W8 | 观测：健康检查、错误页、workspace 404 引导「回 dashboard 新建」 | 手动 + 一条 e2e smoke |

**注意**：W6 可能涉及 **v3 → v6 数据库/用户数据**；若生产库仍是 notebook 时代 schema，需 migrations 与回滚预案（单独运维单，不阻塞本机 W1–W2）。

### 4.5 本机开发默认（写入团队习惯）

```bash
# 终端 A：avrag-rs（示例端口 8081，避免与其它服务冲突）
# 终端 B：
# frontend_next/.env.local
#   API_PROXY_TARGET=http://127.0.0.1:8081
#   NEXT_PUBLIC_PRICING_REVAMP_ENABLED=1
pnpm dev
```

---

## 5. 工程架构：单源 Token

```text
建议新建（二选一）：
  A) /home/chuan/cos-design-tokens/          # 跨仓库
  B) context-osv6/packages/cos-tokens/       #  monorepo 内，其它站 git submodule 或 copy script

交付物：
  tokens.css              # CSS 变量唯一真相（含 light/dark）
  tokens.tailwind.cjs     # Landing / Why 的 theme.extend
  mark.svg / ContextOsMark 规范
  shell-nav 规范（React + Ghost hbs 镜像）
  README：禁止硬编码清单
```

**消费方**

| 站 | 接入方式 |
|----|----------|
| v6 `frontend_next` | `@import` / 替换 `app/design-tokens.css` |
| landing | Tailwind extend + globals |
| whyiamright | 同上 |
| context-os-theme | `assets/css/brand.css` 由 tokens 生成或 symlink 构建 |
| cchess | 仅 chrome 变量；棋盘色留本地 `--board-*` |

---

## 6a. Next 产品 UX 反馈清单（2026-07-10 用户 + 截图）

> 截图源：  
> - `1.png` Dashboard 顶栏用量  
> - `2.png` 账单 Portal 报错  
> - `3.png` RAG JSON / 提示词库 / 模式条  
> - `4.png`–`6.png` **Grok chat 参考**：思考态 · 检索过程流 · 多步骤工具时间线 · 底栏胶囊 composer  
> 与 §3 视觉基准正交：**可先修交互与正确性，不依赖 Monochrome Ink 全量上线**。  
> **Chat 体验对标**：xAI Grok Web Chat（过程流 + 克制 chrome），非照搬暗黑皮肤。

### 总表

| ID | 区域 | 问题 | 期望 | 优先级 | 类型 |
|----|------|------|------|--------|------|
| **U1** | Dashboard 顶栏 | 中央展示 **5h / 7d** 用量（`UsageMeter` compact） | **删除**顶栏用量块；顶栏只留品牌 + 导航/账号 | P0 | UI 删减 |
| **U2** | 「设置」语义 | Dashboard「设置」→ `/settings?tab=profile`；Workspace「设置」→ 主题/外观下拉 | **统一命名与去向**：账户/资料进 Settings；主题/外观单独「外观」或放在 Settings → Appearance；禁止同名不同行为 | P0 | IA / 文案 |
| **U3** | Settings 导航 | `settings?tab=profile` **无返回 Dashboard** 路径 | 顶栏/页头提供 **返回工作区列表**（`/dashboard`）及当前上下文面包屑 | P0 | 导航 |
| **U4** | 账单 · 管理计划 | 点「管理计划」红条：`Self-service billing portal is unavailable; manage subscriptions via Creem or contact support` | ① 未接 Portal 时按钮 **禁用/隐藏** 并中文说明原因与替代路径（升级页 / 外链 Creem）；② 已配置则跳转 Portal；③ 错误文案 i18n，禁止生硬英文甩锅 | P0 | Billing 体验 + 配置 |
| **U5** | 用量模型混乱 | 并存：**5h/7d 窗口**、**令牌/文档**计数、「可用方案」塞在 billing 折叠区 | **信息架构收敛**（见下 §6a.1）；可用方案 **独立页或全屏/大弹窗**，不 nested 在账单卡片底部 | P0 | IA |
| **U6** | 左栏 Session | 会话列表 **无可用滚动条**（长列表难扫） | `overflow-y: auto` + 可见滚动（thin scrollbar）+ 列表区 `min-height: 0` 参与 flex 收缩 | P1 | CSS |
| **U7** | 提示词库 | 左下「提示词库」整块（含搜索/清空） | **整功能删除**（UI、store、e2e、文案）；产品不再提供 | P0 | 删功能 |
| **U8** | 四模式 | 模式切换 **不够显著**；且需 **点击展开再点选项**（多一步） | 见 **U8′ / U14**：悬停即出菜单 + 选中态清晰 | P0 | 交互 |
| **U9** | 新会话闪旧内容 | 新建会话发首条消息时，chatboard **先闪上一轮 session 正文**，新内容到了才切走 | 新建 / `activeSessionId=null` 时 **立即清空 transcript**；stream 未绑定新 session 前不展示旧 messages；加回归测试 | P0 | Bug |
| **U10** | RAG 回复形态 | RAG 主区出现 **`doc_profile` 工具 JSON 代码块** 当作回答（截图 3） | 工具结果进 **活动时间线 / 可折叠 tool card**，主气泡只渲染 **助手自然语言**；禁止把 raw tool payload 当 final answer 展示 | P0 | Bug / 渲染 |
| **U11** | Chat 流式体感 | 现网 chatboard **不像真流式**（块状跳出 / 过程与答案糊在一起 / 缺「思考·检索·汇总」递进） | **Grok 式过程流**（§6a.6）：先状态行 + 可展开步骤时间线（计时、已浏览/已搜索、结果数），再 **token 级流式**写出最终答案；光标/尾闪可选 | P0 | 流式 UX |
| **U12** | Chat 视觉与交互距最佳实践 | 图标、密度、气泡、composer、进度卡与 Grok/Claude/ChatGPT 级产品差距大 | **Chat UI 精修包**（§6a.7）：线框图标、信息层级、底栏胶囊输入、克制进度样式；对齐 STYLE_BASELINE，不抄暗黑皮 | P1 | 设计系统落地 |
| **U13** | 字号偏大 | 全局 / chat 正文字号、标题、会话列表 **整体偏大**，板面挤、不像工具产品 | 下调 chat 与 shell 字阶一档（§6a.8）；与 STYLE_BASELINE 同步改默认 body/control | P1 | 字阶 |
| **U14** | 模式选择交互 | 模式需点击触发菜单 | **hover（桌面）即弹出**选项列表；移出延迟关闭；键盘/触屏仍支持 click；选中后菜单收起 | P0 | 交互 |

### 6a.1 用量与方案信息架构（U5 细化）

**现状问题**

- Dashboard 顶栏：滚动窗口额度（5h/7d）  
- Settings → 账单：令牌 / 文档 + 可用方案折叠  
- Workspace 另有 usage toast / paywall  

**目标模型（推荐）**

| 概念 | 用户可见名 | 放哪 |
|------|------------|------|
| 周期额度 | 「近 5 小时 / 近 7 天」使用量 | **仅** Settings → 用量（或独立 `/settings?tab=usage`），**不要** Dashboard 顶栏 |
| 资源计数 | 令牌、文档（若产品仍计量） | 与周期额度 **同一「用量」页**，分区标题写清「窗口额度」vs「资源配额」，避免两套数字无说明 |
| 订阅方案 | Free / Pro … | **「升级 / 方案」独立路由或大弹窗**（复用 `/upgrade` 或 `PricingCards`），账单页只保留：当前方案摘要 +「更换方案」入口 + 发票/Portal（若有） |

**账单页精简**

```text
账单与计划
  · 当前方案 / 状态 / 续费日
  · [更换方案] → 独立页或 modal（非页内折叠列表）
  · [管理订阅] → Portal（不可用则隐藏+说明）
用量（可同页下半或 tab）
  · 5h / 7d 进度
  · 令牌 / 文档（若保留）
```

### 6a.2 设置信息架构（U2/U3 细化）

| 入口位置 | 控件文案 | 目标 |
|----------|----------|------|
| Dashboard 顶栏 | **账户** 或头像 | `/settings?tab=profile` |
| Workspace 顶栏 | **账户** | `/settings?tab=profile`（或菜单：账户 / 外观） |
| Workspace 顶栏 | **外观**（若需快速切主题） | 仅 theme/locale 下拉；**不要**叫「设置」 |
| Settings 任意 tab | **← 工作区** | `Link` → `/dashboard` |

### 6a.3 提示词库删除范围（U7）

| 路径 | 动作 |
|------|------|
| `components/workspace/workspace-query-library-panel.tsx` | 删除挂载 |
| `lib/workspace/query-library/**` | 删除或标记 dead（优先删） |
| `workspace-history` / shell 左栏布局 | 去掉底部库区域，会话列表吃满高度（利好 U6） |
| i18n `workspaceQueryLibrary*` | 删除 |
| `e2e/specs/smoke/query-library.spec.ts` + 相关 unit | 删除或永久 skip 并说明产品下线 |
| localStorage `context-os.query-library.v1` | 不再读写（可忽略遗留 key） |

### 6a.4 新会话闪旧内容（U9）根因方向

| 嫌疑点 | 文件 |
|--------|------|
| `startNewThread` 只 `setActiveSessionId(null)`，未 `messageHistory.reset()` | `use-workspace-data` / `use-chat-session` |
| stream 开始前仍绑定旧 `sessionId` 的 messages | `hooks/chat-session/*` |
| UI 在 `sessionId` 切换过渡帧未 empty-state | `workspace-chat-pane` / `chat-message-list` |

**验收**：点「新建会话」→ transcript 空 → 输入发送 → 仅出现本轮 user + streaming assistant，无旧问答。

### 6a.5 RAG JSON 主区（U10）根因方向

| 嫌疑点 | 说明 |
|--------|------|
| tool_result / `doc_profile` 被当成 assistant markdown | `chat-message-list` / `tool-result-card` / citation-renderer |
| 后端把 tool payload 写入 `content` 而非 `tool_results` | 需对照 stream 事件；前端兜底：识别 JSON tool dump 降级为折叠卡 |
| Write/RAG 管道混用展示 | 保证 rag 最终 answer 字段优先 |

**验收**：RAG 问答主气泡为自然语言；工具细节仅在「知识库检索」时间线或可折叠卡中。

### 6a.6 Grok 式流式与过程流（U11）— 参考截图 4/5/6

**参考行为（Grok Web，用户提供截图）**

| 阶段 | 用户看到的 | 我们应对齐的点 |
|------|------------|----------------|
| 发出后立刻 | 用户气泡已固定在上；主区出现 **低对比状态行**（如 `Thinking about your request · 1s`）+ 微动效点阵 | 不是空白干等，也不是大卡片堵屏 |
| 工具/检索中 | 状态行升级为步骤标题 + **耗时**（`Analyzing… · 31s`）；下方 **可展开/折叠的条目列表**（已浏览链接、已搜索 query、结果数 pill） | 进度是 **时间线**，不是一整坨 markdown |
| 汇总中 | 步骤文案切换（`Summarizing… · 42s`），子项继续追加 | 多阶段可串行展示 |
| 最终回答 | **流式逐字/逐 token** 出现在助手区；过程块可保留折叠在答案上方 | 答案与过程 **分层**，过程不冒充正文 |

**现状差距（Context OS）**

- SSE 可能已到前端，但 UI 更像「整段替换 / 大卡片 progress」，缺少 **状态行 → 步骤树 → 流式正文** 三段式  
- 工具 payload 易泄漏进主区（与 U10 叠加）  
- 无统一的「进行中」尾光标或细进度反馈  

**实现要求**

| 项 | 规范 |
|----|------|
| 传输 | 保持真 SSE；禁止「等整包再 setState 一次」的伪流式（若后端 chunk 过粗，前端可做 **平滑揭示** 但需标注非伪造延迟过长） |
| 状态行 | `progress` / `thinking` / tool 事件 → 单行 status + elapsed timer |
| 步骤列表 | 每条 tool/search：图标 + 短标题 + 可选 URL/query + 结果数；默认展开最近阶段，完成后可折叠 |
| 最终 answer | 仅 `answer` / assistant content delta 写入主气泡；**token 追加渲染** + 可选 caret |
| 完成态 | timer 停；status 收成「已完成」或直接收起为可展开「过程」摘要 |
| 无障碍 | `aria-live="polite"` 更新 status，不每 token 刷整页 live |

**验收**

- 发送后 ≤200ms 出现 thinking/status 行  
- 检索类模式可见步骤条目与耗时（有事件则必显）  
- 正文可见 **持续增长** 而非一次蹦出长段  
- 与 U10 同时：主区无 raw JSON  

### 6a.7 Chat UI 最佳实践精修包（U12）

对标 Grok/Claude/ChatGPT 的 **交互与密度**（色彩仍跟 Monochrome Ink，不强制 Grok 纯黑）：

| 维度 | 问题倾向 | 目标 |
|------|----------|------|
| 图标 | 粗、不统一、装饰过重 | 统一 **1.5–1.75 stroke** 线框 SVG 集；同一栏同尺寸 |
| 气泡 | 过宽/过圆/阴影脏 | 用户气泡轻底；助手 **无重卡片感**；最大宽 ~48rem |
| Composer | 多行控件碎 | 底栏 **单条胶囊/圆角条**：左附件 · 中输入 · 右模式+发送；参考 Grok 底栏信息架构 |
| 进度 | 大块 card 抢主阅读区 | 进度 **内嵌于 transcript 流**，字号小于正文、颜色 muted |
| 侧栏 | 重、字大 | 会话行更紧：单行标题 + 弱 meta；hover 才露操作 |
| 空态 | 弱 | 居中短提示 + 模式暗示，无巨型插画 |
| 动效 | 跳或无 | 入场 fade 短；stream caret 可选；`prefers-reduced-motion` 关掉装饰 |

**不在 U12 范围**：换引擎模型、重做后端 agent 拓扑。

### 6a.8 字号下调（U13）

相对当前 v6 默认，**Chat / Shell 优先**下调（STYLE_BASELINE 同步）：

| Token | 现行约 | 目标约 | 场景 |
|-------|--------|--------|------|
| `--font-size-body` | 0.9375rem (15) | **0.875rem (14)** | 助手正文、通用 UI |
| `--font-size-control` | 0.875rem | **0.8125rem (13)** | 按钮、输入、模式 |
| `--font-size-meta` | 0.8125rem | **0.75rem (12)** | 会话 meta、进度行 |
| `--font-size-section-title` | 1rem | **0.9375rem** | 侧栏区标题 |
| 会话列表标题 | 偏 body | **13–14px / medium** | 左栏 |
| 进度/status 行 | 偏 body | **12–13px / muted** | Grok 式过程字 |

页级 Marketing H1 可保持较大；**产品壳不要用 marketing 标题尺**。

### 6a.9 模式选择：Hover 打开（U14，修订 U8）

| 项 | 规范 |
|----|------|
| 桌面指针 | **mouseenter** 打开菜单（可 0–100ms 防误触）；**mouseleave** 延迟 150–300ms 关闭（经过菜单桥梁不关） |
| 点击 | 仍可 toggle（触屏 / 键盘主路径） |
| 触屏 | tap 打开；点外侧关闭（无可靠 hover） |
| 键盘 | `Enter`/`Space` 打开，方向键选择，`Escape` 关闭（保留现有） |
| 选中 | 点选项立即 `applyModeSelection` 并关闭；当前项 check / 墨底 |
| 可见性 | 触发器展示 **当前模式短名 + chevron**；菜单项四模式全名+一行微说明（可选） |

**与 U8 合并验收**：不靠「先点开再找字」，悬停即见四模式；选中态在收起触发器上仍可辨。

---

## 6. 分波次开发计划

### Wave 0 — 决策冻结（0.5 天）

- [x] 基准：v6 结构  
- [x] 弃用青 / 绿  
- [ ] 确认标志色：**Copper Ink**（默认）或 **纯灰 Accent**  
- [ ] 确认 Logo：ContextOsMark only  
- [ ] 确认 Workspace 优先：**本机 W1–W2** 与 **生产 W3–W6** 排序（建议先本机再生产）  
- [x] 产品 UX 反馈 U1–U14 已入库（§6a，含 Grok 流式对标）

### Wave 1 — Workspace 连通（P0，1–3 天）

目标：人能打开真实 workspace 并对话。

1. W1–W2 本机闭环  
2. W3–W5 生产切换（可与 Wave 2 部分并行，但 **未完成前不做公域大改上线**）  
3. W7–W8 配置与引导  

**出口标准**

- 本机：dashboard → workspace → chat 通路  
- 生产：`/api/v1/workspaces` 返回 JSON（401/200），不再是 Next HTML 404  

### Wave 2 — Token 换血（v6 先，1–2 天）

1. 重写 `design-tokens.css` 为 Monochrome Ink + Copper  
2. 删除 / 映射旧 cyan、workspace 重复色相  
3. 主 CTA = 墨色；accent 仅 link / focus / chip  
4. 全局替换硬编码阴影与 `#10b981` / cyan  
5. `ContextOsMark` 主题化  
6. `prefers-reduced-motion` 全局一条  

**出口标准**

- 亮/暗切换无青/绿块残留  
- `pnpm typecheck` + 关键页面目视  

### Wave 3 — v6 产品 UX 反馈修复（优先于纯美化，2–5 天）

依据 **§6a 用户反馈清单** 落地，顺序建议：

| 批次 | ID | 说明 |
|------|-----|------|
| 3A 导航与壳 | U1 U2 U3 | 去掉顶栏用量；统一「设置」语义；Settings 回 Dashboard |
| 3B 会话与聊天 | U9 U6 U7 U14 U8 | 新会话闪旧；滚动；**删提示词库**；模式 **hover 弹出** + 选中显著 |
| 3C 流式与 RAG | U11 U10 | **Grok 式过程流 + 真流式正文**；工具 JSON 不进主气泡 |
| 3D Chat 精修 | U12 U13 | 图标/composer/密度最佳实践；**字号下调** |
| 3E 账单信息架构 | U4 U5 | Portal 错误；用量模型收敛；方案独立入口 |

**出口标准**

- §6a U1–U14 均有对应提交与手工验收  
- 提示词库 UI 与 e2e `query-library` 规格移除或改 skip  
- 新会话首条消息过程中 transcript **不出现**上一会话正文  
- 发送后可见 status/步骤流，正文 **增量增长**（U11）  
- 桌面模式选择 **悬停即出** 四选项（U14）

### Wave 3.5 — v6 体验美化对齐（可与 3 并行收尾，1–3 天）

1. EmptyState + Skeleton 组件（Dashboard / Chat / Sources）  
2. Settings / API Access / Admin 去 inline：抽 `Stack` / `Panel` / `Field` / `SectionHeader`  
3. 中文标题字阶规则落地  
4. mono 统一 JetBrains  
5. 收敛 `globals.css` 分文件（base / chrome / dashboard / motion）  

### Wave 4 — 公域三站换肤（2–3 天）

1. Landing + Why 接 tokens + 新 mark + nav  
2. Ghost theme 再生 `brand.css` 并 upload  
3. 互链与 footer 文案统一  
4. 截图回归（homepage / article / why）  

### Wave 5 — 象棋纳入家族（1 天）

1. Cos 顶栏/页脚（纸色主区保留）  
2. 按钮圆角 8px；字号 token  
3. canju 链入家族工具列表  

### Wave 6 — 硬化（持续）

1. token 变更 checklist  
2. 可选 visual regression（Playwright screenshot 关键页）  
3. 文档：本文件 + 更新旧 visual-overhaul 为「已被 Monochrome Ink 取代」  

---

## 7. 任务拆分表（执行用）

| ID | Wave | 仓库 | 内容 | 依赖 |
|----|------|------|------|------|
| T-W1 | 1 | avrag-rs / frontend_next | 本机 API 代理与后端启动 | — |
| T-W2 | 1 | VPS | 部署 avrag-rs + nginx 切换 | T-W1 |
| T-W3 | 1 | VPS | 退役 app 域名上的 v3 3002/3003 | T-W2 |
| **T-U1** | **3A** | frontend_next | **U1** 移除 Dashboard 顶栏 UsageMeter | — |
| **T-U2** | **3A** | frontend_next | **U2/U3** 设置/账户命名 + Settings 返回 Dashboard | — |
| **T-U7** | **3B** | frontend_next | **U7** 删除提示词库（含 e2e） | — |
| **T-U9** | **3B** | frontend_next | **U9** 新会话清空 transcript（+ 测试） | — |
| **T-U6** | **3B** | frontend_next | **U6** 会话列表可滚动 | T-U7 后列表更高，一并验 |
| **T-U14** | **3B** | frontend_next | **U14/U8** 模式菜单 hover 打开 + 选中显著 | — |
| **T-U11** | **3C** | frontend_next (± stream 事件) | **U11** Grok 式过程流 + 真流式正文 | 可与 T-U10 同 PR |
| **T-U10** | **3C** | frontend_next (± backend) | **U10** RAG 不展示 raw tool JSON 作答 | 可先前端兜底 |
| **T-U12** | **3D** | frontend_next | **U12** Chat 图标/composer/密度精修 | T-U11 后或并行 |
| **T-U13** | **3D** | frontend_next + STYLE_BASELINE | **U13** 产品壳字号下调一档 | 可与 T-V1 合并 |
| **T-U4** | **3E** | frontend_next (± billing 配置) | **U4** 管理计划 Portal 不可用态 | — |
| **T-U5** | **3E** | frontend_next | **U5** 用量 IA + 方案独立页/弹窗 | T-U1 |
| T-V1 | 2 | frontend_next | design-tokens Monochrome Ink | Wave0 色确认 |
| T-V2 | 2 | frontend_next | CTA / focus / 去青去绿扫尾 | T-V1 |
| T-V3 | 3.5 | frontend_next | Empty/Skeleton + 副屏 primitive | T-V1 |
| T-V4 | 4 | landing / why / theme | 接 token + logo | T-V1 |
| T-V5 | 5 | cchess | shell only | T-V1 |
| T-S1 | 2–5 | cos-tokens | 单源包 + 同步脚本 | Wave0 |

---

## 8. 明确不做（本计划范围外）

- 不为统一而重写象棋棋盘材质色  
- 不把产品默认改成全站 `#0a0a0a`（工作台保持亮色默认）  
- 不引入新 UI 框架（shadcn 全家桶等）— 先吃透 token + 少量 primitive  
- 不在未部署 avrag-rs 前「修前端 workspace mock」假装连通  
- **不恢复提示词库**（U7 为永久下线，除非产品再次立项）  
- 不在 Dashboard 顶栏重新塞用量（U1）；用量只在 Settings/用量页  

---

## 9. 验收清单

### 连通

- [ ] 本机 workspace 可加载 sessions 并可发消息  
- [ ] 生产 `app.contextlm.top/api/v1/workspaces` 为 JSON  
- [ ] 象棋 `canju` 与 App API 端口隔离  

### 产品 UX（§6a）

- [ ] **U1** Dashboard 顶栏无 5h/7d 用量块  
- [ ] **U2** 无「同叫设置、行为不同」；账户 vs 外观分离  
- [ ] **U3** Settings 任意 tab 可一键回 `/dashboard`  
- [ ] **U4** 管理计划：Portal 可用则跳转；不可用则无英文红条惊吓，有中文说明  
- [ ] **U5** 周期额度 / 资源配额 / 方案 分区清晰；方案不挤在账单折叠底  
- [ ] **U6** 会话列表长列表可滚且滚动条可见（或系统 overlay 可滚）  
- [ ] **U7** 无提示词库 UI；相关 e2e 已删  
- [x] **U8/U14** 桌面悬停即出四模式；选中态收起后仍可辨  
- [ ] **U9** 新建会话首条消息不闪旧 transcript  
- [ ] **U10** RAG 主区为自然语言，无 `doc_profile` 类 JSON 当答案  
- [x] **U11** 有 thinking/步骤时间线 + 正文增量流式（对标 Grok 过程感）  
- [x] **U12** Chat 区图标/composer/密度达到约定精修标准  
- [x] **U13** 产品壳默认字号已按 §6a.8 下调  

### 视觉

- [ ] 全站无品牌青绿（允许 semantic 低饱和 success）  
- [ ] 主按钮墨色；accent 仅点缀  
- [ ] 唯一 ContextOsMark  
- [ ] Landing / Blog / Why 顶栏一致  
- [ ] Canju 有家族壳  

### 工程

- [ ] 单一 tokens 源  
- [ ] 无新增 `rgba(15,23,42` 阴影  
- [ ] 关键路径无大片 inline layout  

---

## 10. 建议立即执行顺序（Solo）

```text
今天：  T-U1 去顶栏用量 · T-U7 删提示词库 · T-U9 新会话清空
接着：  T-U14 模式 hover · T-U6 列表滚动 · T-U2/U3 导航
重点：  T-U11 Grok 式过程流+真流式 · T-U10 RAG 分层（Chat 核心体验）
然后：  T-U12/U13 精修与缩字号 · T-U4/U5 账单
并行：  T-W1 本机 backend（workspace 连通）
本周：  T-W2/T-W3 生产 API · T-V1 token 换血（可与 U13 合并改字阶）
随后：  Wave 3.5 → Wave 4 公域 → Wave 5 象棋
```

**说明**：U1/U7/U9/U11/U14 **不依赖** 色彩换血，应插在视觉 Wave 2 之前或并行，避免「先刷漆后修门」。U11 是 Chat 体感主矛盾，优先于公域换肤。

---

## 11. 附录 A — 与旧文档关系

| 文档 | 关系 |
|------|------|
| `frontend_next/docs/superpowers/specs/2026-04-27-visual-overhaul-design.md` | 结构/字阶仍有效；**青色主轴作废**，由本文 Monochrome Ink 取代 |
| 本文 | **多站点 + 色彩 + Workspace 运维** 的现行计划 |

## 附录 B — 生产 Nginx 目标示意

```nginx
# app.contextlm.top（示意，非直接粘贴上线）
location /api/ {
    proxy_pass http://127.0.0.1:8081;  # avrag-rs，勿用 cchess 8080
    proxy_http_version 1.1;
    proxy_set_header Connection "";
    proxy_buffering off;               # SSE
    proxy_read_timeout 3600s;
}
location / {
    proxy_pass http://127.0.0.1:3000;  # v6 Next standalone
}
```

## 附录 C — 标志色对照（记忆）

| 旧 | 新 |
|----|-----|
| v6 cyan accent | Copper / 或中灰 |
| 公域 `#10b981` | 同上 |
| CTA 黑→青 hover | CTA 始终墨色；hover 加深/提亮灰 |
| 绿块 logo | ContextOsMark |

---

**文档结束。** 下一步：确认 §3.2 标志色（Copper vs 纯灰）后，可按 §10 从 T-W1 + T-V1 开干。
