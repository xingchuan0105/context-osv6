# G5 旅程对照（Next 基线 `5f9dedda` vs Rust）

口径：完成态一致即可，不要求像素或逐步点击数完全相同。Next 规格以冻结基线路径为准；Rust 以现行 fixture 具名测试为准。

| Job | 成功态（PRODUCT_IA §1） | Next 基线规格 | Rust fixture | 步骤数（Next → Rust） | 完成态 |
|---|---|---|---|---|---|
| J0 | 不建工作区直接提问；本会话文件可引用 | `e2e/specs/journey/chat-session.spec.ts`（多轮）+ session files 在 desktop/chat 旅程 | `g5-journey.spec.ts` J0 + `chat-journey` 会话文件 | 提问→回答 / 上传→就绪→发送 → 同序 3 步 | 个人 `/chat`，RAG 仅本会话文件，无 workspace |
| J1 | 建库 + 工作台资料 | `e2e/specs/journey/workspace-crud.spec.ts` 创建 | `g5-journey.spec.ts` J1 + `workspace-journey` 工作台 | 创建 1 步 + 进入工作台 → 创建并跳转工作台 | `/dashboard/:id` 可见侧栏 |
| J2 | Provider 可用 | settings providers 面板（Next settings 旅程） | `g5-journey.spec.ts` J2 + `auth-settings-journey` BYOK | 填 key → 已配置 → 可撤销 | `/settings?tab=providers` 状态切换 |
| J3 | 余额/升级通道 | `e2e/specs/billing/pricing-page.spec.ts` 档位+同意+checkout | `g5-journey.spec.ts` J3 + `billing-journey` | 见档位 → 同意 → 结账入口 | `/pricing` 出支付对话框 |
| J4 | 分享开启；访客可浏览/提问 | `e2e/specs/journey/workspace-share.spec.ts` enable + 访客只读 | `g5-journey.spec.ts` J4 + `share-invite-journey` | 开链 → 访客打开 | 分享中心链接 + `/shared/kb` 含 `ChatPage` |
| J5 | 汇总或单库趋势 | analyze / share analytics（Next analyze-workflow） | `g5-journey.spec.ts` J5 | 单库 analytics + 全局 `/dashboard/analytics` | 图表/浏览量来自 API |
| J8 | 设置完成目标项 | `e2e/specs/smoke/auth-flow.spec.ts` 注册登录；settings 安全 | `g5-journey.spec.ts` J8 | 资料/安全 tab → 登出 | `/settings` 面板 + `/login` |
| 管理台 | 巡检可达 | `e2e/specs/smoke/admin-navigation.spec.ts` | `g5-journey.spec.ts` 管理台巡检 + `admin-journey` | 概览入口网格 | `/admin` 入口卡可见 |

Next 侧本机未跑冻结基线进程（`:3000` 不是 Next）。对照证据：上表规格路径 + `5f9dedda` 视觉快照 `login`/`dashboard`（见 `next/`）。
