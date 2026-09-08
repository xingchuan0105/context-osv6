# G5 总门证据（2026-09-08）

## 结论

**不能盖章 G5 达成。** 旅程双轨、C1、回归、E5.1–E5.5 证据齐全；§1 终态条件 2（CSS 深度 ≥ Next 基线 80%）未达，见 `CSS_COUNTS.md`。W5 前置仍为 G3/G4，不改为 G3/G4/G5。

评审人走查签字（结构/状态/密度，非像素）：

- 日期：
- 结论：通过 / 有条件通过 / 不通过
- 签字：

## 1. 旅程

| 轨 | 结果 |
|---|---|
| Fixture `g5-journey.spec.ts` | **8/8** J0–J8 + 管理台 |
| Fixture 全套 | **100 passed / 7 skipped**（skip = live 套件未开 `LIVE_BACKEND`） |
| Live `LIVE_API_BASE=http://127.0.0.1:18090` | **9/9**（原 chat-live 2 + G5 live 7） |

对照表：`MATRIX.md`。Next 进程本机不在跑；基线对照用 `5f9dedda` 规格路径 + 视觉快照。

## 2. 感知走查截图

Rust fixture 1280×800：`rust/{chat,dashboard,workbench,settings,pricing,login,share,admin}.png`

Next 基线快照（`5f9dedda`）：`next/{login,dashboard}.png`

## 3. 机械门

- C1：`style_baseline_guard` 全绿（字重 / 裸 hex / 阴影白名单 / 无内联 style / 悬空 var=0）
- 占位清零：`"98.5"` / `"42"` / `"128"` 在 `frontend_rust` 源码 grep=0（条件 3）
- 条件 2：§0 **出现次数** 80% 仍未达；**唯一语义** `@keyframes` 14/13、`:focus` / `:active` 已超 Next 源码唯一基。口径说明见 `CSS_COUNTS.md`。未改 W5 前置。
- 视觉深度补丁（本波）：把 Next 13 个唯一动效接到真实表面（thinking / chase / status spin / shimmer / step-in / panel-in）；控件 hover/active/focus 矩阵补齐 CTA、composer、资料 processing、菜单面板。接受细微偏差（progress 为条带而非 3×3 点阵）。
- Leptos 官方默认就是全局 CSS；不上 Tailwind/Stylance 灌选择器。
- E5.1–E5.5 证据目录仍在

## 4. 回归

- `cargo test -p web-sdk -p web-ui`：通过
- fixture 100 绿
- live smoke 9 绿
- G2–G4 既有证据未作废
