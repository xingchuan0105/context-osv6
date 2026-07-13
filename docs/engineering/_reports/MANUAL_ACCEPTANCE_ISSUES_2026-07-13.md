# 人工验收问题记录（2026-07-13）

| 字段 | 值 |
|------|-----|
| 日期 | 2026-07-13 |
| 环境 | local product-dev-up · `http://127.0.0.1:3000` |
| 工作区 | `9e3abf9d-cae9-43d2-882c-d27c05969c66`（及同会话其它路径） |
| 状态 | **记录 / 诊断** — 暂不修复（用户拍板） |
| 截图 | `E:\OneDrive\桌面\1–5.png`（WSL: `/mnt/e/OneDrive/桌面/`） |

---

## 0. 术语表（沟通用）

| 口语 | 推荐正式叫法 | 本产品代码/testid 对应 | 说明 |
|------|--------------|------------------------|------|
| 「正在处理 / 思维过程」 | **流式进度反馈**（streaming progress） | `workspace-progress-card`（research 模式：RAG/Search/Write） | 发送后立刻出现的阶段性状态卡（检索中、工具调用中、写稿中…） |
| 同上（非 research 短提示） | **状态提示条**（status hint） | `workspace-status-hint` | 通用 chat 等紧凑进度 |
| 进度时间线 | **活动时间线 / Agent activity timeline** | `progress-timeline.tsx` · `workspace-progress-elapsed` | 多步活动列表 + 耗时 |
| 答案区闪烁光标 | **流式插入符**（stream caret） | `stream-caret` · `message.pending` | assistant 气泡未完成时的光标 |
| 中间思考文案 | **推理过程 / reasoning stream**（若产品暴露 CoT） | （若 SSE 有 reasoning 事件） | 行业亦称 thinking indicator；本产品更偏 **工具/阶段进度** 而非裸 CoT |
| 工具执行条 | **工具调用结果卡**（tool result cards） | `toolResultCard` / 文案如 `user_profile_load`、`网页搜索` | 显示工具名 + OK/失败；**不应**像调试面板一样堆在终稿下方且无折叠默认真值 |
| 引用来源 | **内联引用 / web source chip** | `workspace-citation` · `citation-button`（「N 个来源」） | 与工具卡不同 |

**团队沟通建议：**

- 说「**发送后要先出 progress card / status hint**」，不要只说「思维过程」（易与 LLM CoT 混淆）。  
- 说「**tool result cards 泄漏/过曝**」指截图 3 底部那几条 `user_profile_load` / `conversation_history_load` / `网页搜索`。  
- 说「**stream caret / pending bubble 延迟**」指答案与进度一起蹦出、没有首帧 pending。

---

## 1. Ingestion 卡在「处理中」（docx）

| 项 | 内容 |
|----|------|
| 现象 | 右侧资料一直转圈；用户感知永久 processing |
| 文档 | `数字化转型IT立项报告-合订版V1.0.3-20260709.docx` · `886de4b1-5abc-426f-9206-3a639950ffb7` |
| 根因 | **OfficeService 解析失败**（`Failed to decode office parser docx`）；任务可重试，`attempt_count` 3/5，`available_at` 退避期间 worker `no tasks` |
| UI 债 | 不展示 `last_error` / failed 态 → 假「卡死」 |
| 截图 | `2.png`（深色 spinner 难辨） |
| 严重度 | P0 体验（解析失败）+ P1 UI 反馈 |

详见会话诊断；**非** claim/RLS 回归。

---

## 2. 会话列表条目拉满左栏高度

| 项 | 内容 |
|----|------|
| 现象 | 仅 1 条会话时，条目纵向占满 history rail（截图 `1.png`） |
| 用户预期 | **固定高度** 条目（标题单行/两行截断），列表区滚动，**不要**单条被 stretch 成整栏 |
| 根因（布局） | `.historyList`：`flex: 1` 撑满栏高 + `display: grid` + 默认 `align-content: stretch` → 单行 auto track 被拉长 |
| 位置 | `workspace-shell.module.css` · `.historyList` / `.historyItem*` |
| 严重度 | P1 UI；验收明确要求 **fixed-height items** |

---

## 3. Ingestion spinner 对比度

| 项 | 内容 |
|----|------|
| 现象 | 处理中图标深色底 + 细环，动效看不清（`2.png`） |
| 实现 | `.selectionMark` + `.sourceStatusSpinner`（复用勾选框位） |
| 严重度 | P2 可视；与失败不可见叠加更糟 |

---

## 4. 发送消息后，进度反馈未「先于答案」出现

| 项 | 内容 |
|----|------|
| 现象 | 点击发送后 **没有** 第一时间出现「正在处理 / 进度」类 UI；最终答案（及进度类内容）**一起**出来 |
| 正式叫法 | **流式进度反馈延迟 / missing early streaming progress**（progress card 或 status hint 首帧缺失） |
| 可能涉及 | SSE 首事件过晚；前端仅在有 content/activity 时才 mount `ProgressTimeline`；`message.pending` 气泡未立即插入；search/write 工具阶段未推 activity |
| 期望 | 发送成功后 **≤~100–300ms 体感内** 出现 pending assistant 行和/或 live **progress card / status hint**，再流式追加答案 |
| 严重度 | P1 对话体验（尤其 search/rag/write 长等待） |

---

## 5. Web Search 答案区工具卡过曝（截图 `3.png`）

| 项 | 内容 |
|----|------|
| 现象 | 网络搜索终稿下方露出调试感卡片：`user_profile_load` OK、`conversation_history_load` OK、`网页搜索` OK |
| 正式叫法 | **工具调用结果卡泄漏 / tool-result overexposure**（非用户友好的 internal tool strip） |
| 期望方向（记录，未修） | 默认折叠/隐藏内部工具（profile/history）；用户侧最多「已搜索网页」级摘要；完整工具条留给 debug 或可展开 |
| 另见 | 正文管道表 `| tw 台湾 |` 等更像 **原始检索拼接** 而非润色后的 assistant 文案（search 合成质量/提示词问题，可单列） |
| 严重度 | P1 产品 polish（工具卡）；P2 答案质量（管道格式） |

---

## 6. 旁支（截图 1 聊天区，同次验收）— **非用户主动提报**

| 项 | 内容 |
|----|------|
| Chat 429 文案 | **验收排查时 agent 从截图 1 / API 日志 / 用户偏好记忆字段旁观到**，**不是用户单独提的一条反馈**。历史 chat 失败时 assistant 会回 `General mode is temporarily unavailable… 429… model_not_found`；该串甚至被写入 `frequently_asked_topics`（像「用户说过的话题」）。属 **上游 LLM 饱和/配置** 旁支，**可从用户验收清单撤出**。 |
| 右侧无资料 | 与左侧会话无关；ingest 失败时资料列表可仍空或仅 processing 项 |

---

## 7. 问题清单汇总（修优先级草案）

| ID | 问题 | 期望 | 优先级 |
|----|------|------|--------|
| A | docx Office 解析失败 + UI 假 processing | 失败可见；parser 可排障 | P0/P1 |
| B | session 条目拉满栏高 | **固定高度** 条目 + 列表滚动 | P1（验收明确） |
| C | spinner 对比差 | 高对比 processing 指示 | P2 |
| D | 无早期流式进度反馈 | 发送后立刻 progress/status/pending | P1 |
| E | web_search 工具卡过曝 | 折叠/隐藏内部工具 | P1 |
| F | search 答案管道/原文感 | 合成质量（可后置） | P2 |
| G | ~~chat 429~~ | **非用户反馈**；agent 旁观旁支，见 §6。可忽略/撤出验收 | — |
| H | websearch 答案后「又冒出 query」 | 见 §9 重查（2026-07-13） | P1 |
| I | 刷新后工具摘要全展开、难折叠 | 见 §10 诊断 | P1 |
| J | 账单页「更换方案」与「管理订阅」双入口 | 见 §11；验收倾向只留一个 | P2 产品 |
| K | 登录页无「忘记密码」 | 见 §12；能力位关 + SMTP 未配齐 | P1 配置/产品 |
| L | Dashboard 顶栏「账户」与头像入口重叠 | 见 §13；均进 profile | P2 产品 |

---

## 8. 沟通备忘

- **不要**把 D 叫成「思维链 CoT」除非确实在展示 model reasoning tokens。  
- 本产品更接近：**Agent streaming progress + tool activity**，组件名 **ProgressTimeline / progress card**。  
- B 的验收句：**「history-item 应为固定高度，historyList 滚动，禁止单条 stretch 占满 rail。」**  
- H 的验收句：**「单次发送后 transcript 不得出现第二条同文 user 气泡；assistant 澄清问句不得被误认为 user query。」**  
- I 的验收句：**「刷新后 tool result 默认折叠；网页搜索摘要不得默认铺开全文。」**  
- J 的验收句：**「设置 → 账单 只保留一个订阅/方案主 CTA（更换方案 或 管理订阅），避免双按钮。」**  
- K 的验收句：**「登录页在密码重置可用时应显示「忘记密码」→ /reset-password。」**  
- L 的验收句：**「Dashboard 顶栏只保留一个账户/设置入口（账户文案按钮 或 头像），勿双链同目标。」**

---

## 9. Web Search：答案出现后又出现 query（新增）

| 字段 | 值 |
|------|-----|
| 现象（用户） | websearch 聊天时，答案弹出后 **又出现一个 query**；怀疑答案被吞或其它原因 |
| 正式叫法 | **后置伪 query / post-answer phantom query**（或 **transcript 次序异常 / 澄清问句误读为 user**） |
| 工作区/会话 | `9e3abf9d-…` · session `7e031711-b7af-4c36-8929-016f837e93e6` |
| 证据 | DB `chat_messages` + 截图 `3.png` + API log search preflight |

### 9.1 数据库事实（同会话）

| id | role | agent_id | 内容摘要 | created_at |
|----|------|----------|----------|------------|
| 835 | user | | 你好，在吗 | 13:57:10 |
| 836 | assistant | chat | 在的！… | 13:57:10 |
| **837** | **user** | | **今天天气如何？** | **14:01:34** |
| **838** | **assistant** | **chat** | 纯对话、**不能联网**；建议切 Search | **14:01:34** |
| **839** | **user** | | **今天天气如何？**（与 837 **同文**） | **14:01:59** |
| **840** | **assistant** | **search** | 多地天气表 + 文末 **「问题：请问您在哪个城市？」** | **14:01:59** |

结论：服务端 transcript **确有两条相同 user 文案**，中间夹一条 **chat** 答、再一条 **search** 答（间隔约 25s）。不是单纯前端渲染幻影。

### 9.2 用户可见的两类「又出现 query」（可并存）

| 假设 | 说明 | 证据强度 |
|------|------|----------|
| **H1 · 澄清问句被当成 query** | search 终稿末尾自带 `**问题**：请问您在哪个城市？`（截图 3），形态像「又抛出一个问题」，易被说成「又有一个 query」 | **高**（截图 + msg 840 原文） |
| **H2 · 同文双 user 轮次** | 同一句话进库两次：先 **chat** 一轮、再 **search** 一轮。UI 上会在 chat 答之后再出现一条 user「今天天气如何？」再出 search 答 | **高**（837–840） |
| **H3 · 答案被吞** | 若只看到后一轮 search，前一轮 chat 答可能被滚走/不注意；或 progress 与终稿同帧导致「只见终稿」 | **中**（与 D 叠加） |
| **H4 · done 后 session 重载重排** | `done` → `setActiveSessionId` / `onSessionChange` → `useChatSession` 依赖 `sessionId` **整表 loadSession** 替换本地 optimistic 列表，可能造成气泡「突然多一条/次序跳变」 | **中**（代码路径存在；本例 DB 已有双 user，不全靠前端） |

### 9.3 可能根因拆解

1. **模式不一致双发（H2 主因候选）**  
   - 用户意图 search，但第一次发送时 `effectiveChatMode` 仍为 **chat** → 838 明确「没法实时查询」。  
   - ~25s 后同一文案以 **search** 再发一次 → 840。  
   - 触发可能：手动再发、模式切换后重发、连点、或 UI 在切换模式时未清掉/误触发二次 `send`。  
   - API 在 14:01:45 有 `agent_type=search` preflight 与 query「今天天气如何？」一致。

2. **Search 合成把「澄清问题」写进 assistant 正文（H1）**  
   - 产品设计上可追问城市，但渲染为正文末行 `**问题**：…`，无独立 UI 组件，与 user 气泡难区分。  
   - 与 E（工具卡过曝）同属 **search 终稿呈现过「过程感/调试感」**。

3. **流式时序（H3/H4，与 D 相关）**  
   - 无早期 progress / pending 时，用户只感知「突然整段答案 + 文末问题 + 工具卡」。  
   - `done` 后 reload transcript 会刷新整表，若当时本地只有一轮 optimistic，reload 后出现 **两轮同文** 会更像「答案后又冒 query」。

### 9.4 API 日志重查（定论，2026-07-13）

同一会话 `7e031711…` 对「今天天气如何？」只有 **两次 preflight**（不是一次 HTTP 写两条）：

| UTC | agent_type | 结果 |
|-----|------------|------|
| 06:01:28 | **chat** | 06:01:34 落库 assistant（不能联网，建议切 Search） |
| 06:01:45 | **search** | 06:01:59 落库 assistant（天气表 + 文末「问题：请问您在哪个城市？」） |

间隔约 **11s**。代码路径 **无** chat→search 自动重试；`send` 一次只打一发 stream。第二次一定是 **又一次 stream 请求**（模式已是 search）。

### 9.5 用户不变量 vs 真实代码缺陷（重要）

用户预期：**最新一条不应是「裸 query」；每条 user 后一定有答案位。**

旧前端 **违反** 该不变量：

1. `send()` **只** optimistic 插入 **user**，**不**插 pending assistant。  
2. chat 的 `answer_start` 被 **故意忽略**（`handleAnswerStartEvent` early return），要等 **首 token** 才建 assistant 气泡。  
3. Search 工具阶段（answer 前）可长达十余秒 → UI 底部 **长期只有 user query** → 体感「最新是 query、没有答案」——**机制问题，不是用户错觉**。  
4. 错误/Abort 时 `clearPendingStreamingAssistant` 若尚无 assistant，也会留下 **user 作 tail**。

另：search 终稿 **文末** `**问题**：请问您在哪个城市？` 在视觉上像第二句 query（H1），加重误读。

### 9.6 已修 / 仍开放

| 项 | 状态 |
|----|------|
| 发送立刻插 pending assistant（保证 tail 不是裸 user） | **已修**（`use-chat-stream`） |
| chat 也处理 `answer_start` | **已修** |
| 文末澄清问句独立 UI（H1） | 开放 |
| 双发是否来自切模式后输入框残留 + 再 Enter | 人工/产品侧；代码无自动双发 |

---

## 10. 刷新后「摘要」全展开、无法默认折叠（新增 · 截图 `4.png`）

| 字段 | 值 |
|------|-----|
| 现象（用户） | **刷新页面后**，摘要直接全部暴露，没有办法折叠 |
| 正式叫法 | **工具结果卡默认展开 / tool-result expand-on-reload**（与 E 同属 tool-result 呈现） |
| 截图 | `4.png`：`user_profile_load` / `conversation_history_load` 收起；**「网页搜索」展开**，内含「摘要」+ Brave LLM Context 大段 `[[1]]…[[2]]…` |

### 10.1 这是什么 UI

不是 ProgressTimeline 的「过程摘要」（`workspace-progress-card` 仅在 **当次 stream 会话内存** 中，刷新后 **不存在**）。

而是挂在 assistant 消息上的 **`tool_results` → ToolResultsPanel → ToolResultCard**：

- 持久化：消息入库的 `tool_results`，`loadSession` / 刷新后从 transcript 还原。  
- 组件：`tool-result-card.tsx`。

### 10.2 根因（代码）

```ts
// tool-result-card.tsx
function isCompactToolByDefault(toolName: string): boolean {
  const hint = getToolRenderHint(toolName);
  // JSON / profile / doc_* → 默认折叠
  return hint === "json" || toolName.includes("profile") || toolName.includes("doc_");
}

const [expanded, setExpanded] = useState(() => !isCompactToolByDefault(result.tool));
```

| 工具 | hint / 规则 | 默认 |
|------|-------------|------|
| `user_profile_load` | 含 `profile` | **折叠**（▸） |
| `conversation_history_load` | 未知 → `json` | **折叠** |
| **`web_search` / 网页搜索** | hint=`search` | **展开**（▾）+ 全文「摘要」 |

因此：

1. **每次挂载**（含刷新、重进会话）`web_search` 卡 **强制默认展开**。  
2. 展开体渲染 Brave 返回的 **原始检索摘要**（长英文/多源拼接），视觉上「摘要全暴露」。  
3. Header 上 **可以** `onClick` 切换折叠（chevron ▾/▸），但：  
   - 默认就是开的，刷新即铺满；  
   - 状态 **不持久**（仅 `useState`），一刷新又全开；  
   - 用户体感接近「没法折叠 / 一刷新又摊开」。

### 10.3 与相关项的关系

| ID | 关系 |
|----|------|
| E | 同为 tool-result 过曝；E 偏「内部工具名可见」，I 偏「刷新后 web_search 摘要默认展开」 |
| D | 当次 stream 的 progress 刷新后消失；留下的只有 tool cards |
| F | 摘要内容本身像原始检索 dump，展开后更刺眼 |

### 10.4 修复方向（记录，未开工）

| 方向 | 内容 |
|------|------|
| 默认 | `web_search` 也默认 **compact/折叠**（或仅展示「N 条来源」一行） |
| 持久 | 可选：localStorage 记住 expand；或 transcript 不默认展开任何 tool body |
| 内容 | 摘要区截断 +「展开全文」；完整 Brave dump 仅 debug |
| 产品 | 终稿用 citation chip；工具卡默认隐藏或二级入口 |

### 10.5 严重度

**P1 产品 polish** — 刷新后主阅读路径被检索 dump 淹没；与「答案优先、过程可折叠」预期冲突。

---

## 11. 账单页：更换方案 vs 管理订阅（`/settings?tab=billing`）

| 字段 | 值 |
|------|-----|
| 页面 | `settings-billing-panel.tsx` |
| 用户反馈 | 两者应是同一逻辑，**留一个就行** |

### 11.1 当前实现（并非同一 API）

| 按钮 | 文案 key | 行为 |
|------|----------|------|
| **更换方案** | `settings.billing.changePlanAction` | `<a href="/upgrade">` → 站内升级/选方案页（checkout 路径） |
| **管理订阅** | `settings.billing.managePlanAction` | `POST /api/v1/billing/portal-session` → 跳转 **外部自助门户**（Creem/Stripe Customer Portal：发票、支付方式、取消等） |

同页下方「可用方案」区还有 **第二个「更换方案」** 链到 `/upgrade`（主按钮重复）。

### 11.2 产品判断

- **行业上**：upgrade/checkout 与 billing portal 常拆成两个动作。  
- **本产品 B2C + 门户常不可用**（文案已有 `portalUnavailable`：自助门户未开通时引导去「更换方案」）→ 双按钮易混淆、且 portal 失败时只剩 upgrade 有用。  
- **验收倾向**：设置账单区 **只保留一个主 CTA**。推荐默认保留 **「更换方案」→ `/upgrade`**（可控、与定价页一致）；门户能力若以后要开，可放在 upgrade 页次级链接「管理付款与发票」，而不是设置页并列。

### 11.3 修复方向（未开工）

1. 去掉设置页 **「管理订阅」** 或收成文案链接。  
2. 去掉下方重复的「更换方案」按钮（只留顶部或只留可用方案区一处）。  
3. （可选）有活跃付费订阅且 portal 可用时，再显示次级「账单门户」。

---

## 12. 登录页无「忘记密码」（`/login`）

| 字段 | 值 |
|------|-----|
| 现象 | 登录页看不到「忘记密码」入口 |
| 正式叫法 | **密码重置入口被能力位隐藏**（password-reset capability gate） |

### 12.1 前端其实有入口

`app/(auth)/login/page.tsx`：

```tsx
{passwordResetEnabled ? (
  <Link href="/reset-password">{formatUiMessage(locale, "authForgotPassword")}</Link>
) : null}
```

- `passwordResetEnabled` 来自 `useAuth()`，启动时请求 **`GET /api/auth/capabilities`**。  
- 重置流程页面已存在：`/reset-password` → verify → confirm。  
- 设置页安全区也会按同一能力位显示重置入口。

### 12.2 后端何时为 true

`auth_runtime_capabilities_handler`：

```rust
password_reset_enabled: cfg!(test) || state.password_reset_service().smtp_ready()
```

`smtp_ready()` 要求（`PasswordResetConfig`）：

- `EMAIL_PROVIDER=smtp`（或默认 smtp）  
- `SMTP_HOST` 非空  
- **`SMTP_FROM` 非空**  
- （发信时还需要可用的 `SMTP_USER` / `SMTP_PASS`）

### 12.3 本机实测（验收环境）

```text
GET /api/auth/capabilities → {"password_reset_enabled":false}
.env: SMTP_HOST=smtp.163.com 已设；SMTP_USER / SMTP_PASS / SMTP_FROM = 空
```

→ **能力位 false → 登录页故意不渲染「忘记密码」**。  
不是漏写 UI，而是 **邮件通道未就绪时隐藏**。

### 12.4 处理选项（未替用户写密钥）

| 选项 | 做法 |
|------|------|
| A · 配 SMTP（推荐本地验收） | 在 `avrag-rs/.env` 填 `SMTP_FROM` / `SMTP_USER` / `SMTP_PASS`（及 `RESET_CODE_SECRET` 若空），**重启 avrag-api**，刷新登录页 |
| B · 产品改策略 | 始终展示链接；点进后若 SMTP 不可用再报「暂不可用」（验收可见入口，失败延后） |
| C · 本地 mock 邮件 | 开发专用 provider（若产品后续支持） |

### 12.5 严重度

**P1 验收体验** — 用户以为功能缺失；实际是 **配置门控**。配齐 SMTP 即可显示且走完整邮件重置流。

---

## 13. Dashboard 顶栏：「账户」与头像按钮功能重叠（截图 `5.png`）

| 字段 | 值 |
|------|-----|
| 现象 | 两个控件并排；用户认为功能重叠 |
| 截图 | `5.png`：左侧「账户」+ 人像图标；右侧圆形内字母（如 `X`，实为 **avatarInitial**，非关闭） |

### 13.1 代码事实

`components/dashboard/parts/dashboard-header.tsx`：

| 控件 | 文案 / 展示 | `href` |
|------|-------------|--------|
| 左侧 Link | `dashboardAccountLink` → **「账户」** + 人像 SVG | **`/settings?tab=profile`** |
| 右侧 Link | `dashboardProfileLink` aria-label「账号信息」；可见内容为 **`avatarInitial`** | **`/settings?tab=profile`** |

两者 **同一路由、同一 tab**，无行为差异。

### 13.2 产品判断

与账单双 CTA（J）同类：**重复入口**。  
行业常见只保留 **头像菜单** 或 **「账户」文案按钮** 其一。

推荐（未开工）：

- 只留 **头像**（圆形 initial），点进设置/资料；或  
- 只留 **「账户」** 文案按钮；  
- 若需菜单（登出等），应做成 **一个** 下拉（工作区顶栏 avatar 菜单模式），而不是两个平行 Link。

### 13.3 严重度

**P2 产品 polish** — 不挡功能，但验收噪音、信息架构重复。

---

*记录于人工验收会话；修复需用户另行授权开工。*


---

## 14. 决策落地（2026-07-13 · A–L）

| ID | 用户决策 | 落地 |
|----|----------|------|
| A | 两个都做（UI fail + office 解析路径） | 资料列表展示 failed 徽章 + `last_error`；sources API 带出 ingestion_tasks.last_error；office client 超时默认 120s + 解码错误含 body 预览 |
| B | 单行标题 | history 标题保持单行 ellipsis；`historyList` `align-content: start` 防 stretch |
| C | 与 A 一起改 | processing spinner 提高对比度 |
| D | 4 模式立刻出进度 | 发送时 `progressTracker.show`；chat/write **不再**首 token hide，全部模式保留至 finalize |
| E | 完全隐藏内部工具 | `user_profile_load` / `conversation_history_load` 等从 ToolResultsPanel 过滤 |
| F | 优先自然语言；答案不需要结构化表；点开来源可访问 URL | `search-answer.md` 增加 Answer shape；web_search 卡折叠后展开见链接 |
| G | （问：与哪个反馈相关） | 见 §6 / §7 **G**：Chat 模式 `429` / `model_notfound` — **上游 LLM 配额/模型配置**，非 UI |
| H | 只点一次发送；可能切模式又发；确定没有反复点 | 见 §9：DB 确有双 user 同文（chat→search）。不按连点修；可能是模式切换后二次发送。保留说明 |
| I | 仅 web_search 折叠；完全隐藏内部工具 | 与 E 一并：`isCompactToolByDefault` 仅 search；内部工具不渲染 |
| J | 只留管理订阅 | 设置账单去掉「更换方案」与可用方案升级区，仅 portal「管理订阅」 |
| K | 用户配置 163 SMTP | 写入 `avrag-rs/.env` SMTP_*（不回显密钥）；重启 API 后 `password_reset_enabled` 应 true |
| L | 账户文案 only | Dashboard 顶栏去掉头像链，仅保留「账户」文案按钮 |

