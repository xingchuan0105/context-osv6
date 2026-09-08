# G5 视觉深度计数

## 诊断：缺口不是 Leptos 画不出状态

§1 终态条件 2 原文用「同口径 grep 出现次数 ≥ Next 80%」。这套口径把 Next **CSS Modules 的重复声明**当成深度：

- 同一套 `:hover` / `@keyframes` 在每个 `*.module.css` 里再写一遍，hashed 类名互不相同，出现次数膨胀。
- 计划 §0 的 `@keyframes 33` 对照当前 Next **源码**只有 **13 个唯一名字**（`cardEnter` … `thinkingPulse`）；`statusLineSpin` 与 `sourceStatusSpin` 还是同一条 `rotate(360deg)`。
- Leptos 官方样式路径是 **一份全局 CSS**（book `interlude_styling`：小应用单文件；长大再选 Tailwind / Stylance）。本仓已是 tokens + `assets/style/*.css`，这正是官方默认，不是生态缺陷。
- 上 Tailwind / Stylance 只会把「模块重复计数」搬到 Rust 侧，不增加用户可感知状态。接受视觉细微偏差后，正确口径是 **唯一语义动效 + 控件状态矩阵**，不是选择器出现次数。

## 唯一语义（诚实口径）

Next 源码 13 个 `@keyframes` 名字，Rust 以 14 个唯一名字覆盖（`cos-spin` 同时承担 status/source 转圈，另加 toast / pulse）：

| Next 名字 | Rust 名字 | 接到的表面 |
|---|---|---|
| `cardEnter` | `cos-card-enter` | 工作区卡 / 定价卡 / 用量卡 / 营销卡 |
| `modalEnter` | `cos-modal-enter` | `app-dialog` / web sources |
| `slideInLeft` | `cos-slide-in` | 会话栏 |
| `thinkingPulse` | `cos-thinking` | 进行中 activity 末行 |
| `fadeIn` | `cos-fade-in` | chat 空态 |
| `dashboardSkeletonShimmer` | `cos-shimmer` | dashboard 列表加载 |
| `streamCaretBlink` | `cos-stream-caret` | 流式光标 |
| `progressMatrixChase` | `cos-progress-chase` | 进行中 progress 标题（条带近似，非 3×3 点阵） |
| `messageEnter` | `cos-message-enter` | 消息气泡 |
| `statusLineSpin` | `cos-spin` | 状态行 / 会话文件 / 资料 processing |
| `statusLineSwapIn` | `cos-step-in` | activity 步骤 |
| `sourceStatusSpin` | `cos-spin` | 同上（不重复定义） |
| `editorLinkPanelIn` | `cos-panel-in` | 账户菜单 / 通知面板 / 笔记编辑器 |
| — | `cos-toast-in` / `cos-pulse` | toast、page-status 加载 |

| 信号 | Next 源码唯一 | 80% 门槛 | Rust 唯一 | 相对门槛 |
|---|---:|---:|---:|---|
| `@keyframes` 名字 | 13 | 11 | 14 | 达 |
| `:focus` 选择器基 | 33 | 27 | 50 | 达 |
| `:active` 选择器基 | 18 | 15 | 34 | 达 |
| 产品断点档（640/720/767/768/840 + reduced-motion + color-scheme） | 7 档核心 | — | 7 档核心对齐 | 达 |
| `:hover` 选择器基 | 113（含模块重复类名） | 91 | 67 | 不按此盖章：矩阵覆盖按钮/链接/卡/行/chip，不复制模块类 |

## 出现次数（§0 旧口径，仅对照）

同口径仍取计划 §0 审阅表（2026-09-06，Next 基线 `5f9dedda` 交互/动效行）。

| 信号 | Next 基线（§0） | 80% 门槛 | Rust 现测（`frontend_rust/assets/style`） | 相对门槛 |
|---|---:|---:|---:|---|
| `:hover` | 135 | 108 | 89 | 未达（模块重复） |
| `:focus` | 42 | 34 | 55 | 达 |
| `:active` | 22 | 18 | 34 | 达 |
| `transition` | 54 | 43 | 33 | 未达（全局矩阵合并声明） |
| `@keyframes` | 33 | 26 | 14 | 未达（§0 含重复；源码唯一仅 13） |
| `box-shadow` | 58 | 46 | 29 | 未达（C1 禁止非白名单阴影） |
| `@media` | 42 | 34 | 9 | 未达（Next 多 520/760/800/900/960 等局部断点） |

E5.1 切片底线（`:focus* ≥ 20`、`transition ≥ 20`、`@keyframes ≥ 3`）仍绿；守卫另加 **唯一 `@keyframes` 名字 ≥ 10**。C1 五条 + 悬空 var=0 仍绿。

**G5 总门：** 若仍按 §0 出现次数 80%，条件 2 未达，总门不盖章。若接受「唯一语义 ≥ Next 源码唯一名字 80% + 状态矩阵齐备 + 允许细微偏差」，keyframes / focus / active 已达；hover 不靠灌选择器追 113。W5 前置仍为 G3/G4，除非评审改口径。
