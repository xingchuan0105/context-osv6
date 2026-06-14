# Brooks-Lint Review

**Mode:** PR Review  
**Scope:** v7 计划执行后的 master 三分支 commit（`acda6da` legal / `dfdaac2` app-chat / `317fb3f` hygiene）；复测 `-D warnings`、smoke 守卫、legal P0/P1、desktop/share 测试覆盖。  
**Health Score:** 98/100  
**Trend:** 48 → 98 (+50) vs PR Review v6

**一句话结论：** v6 的两项 Critical（变更集分裂、smoke 清单解析）与四项 Warning 均已闭环；legal 再确认全链路可合并。剩余 2 分：未在本轮重跑完整并行 product smoke E2E；`frontend_next/playwright/` 运行时目录仍 untracked。

---

## Findings

### 🟢 Suggestion

**Coverage Illusion — 完整 product smoke E2E 未在 M15 重跑**

Symptom: M15 验证了 `--check-modules` 守卫、`RUSTFLAGS="-D warnings"` 预编译、transport-http legal 测试、Vitest 293 测；未执行 `./scripts/run-product-smoke-e2e.sh` 全量并行 smoke（需 Docker/外部服务）。

Source: Google — *How Google Tests Software*, change coverage

Consequence: 模块清单与编译门禁已绿，但 auth/share/chat/rag 运行时回归仍依赖 CI 或手动全量 smoke。

Remedy: 合并前在具备依赖的环境跑一次完整 smoke；或在 nightly workflow 留痕。

---

**Change Propagation — `frontend_next/playwright/.auth/` 运行时产物未入库**

Symptom: `git status` 仍显示 `?? frontend_next/playwright/`（Playwright auth state / run-id）。

Source: Feathers — change set hygiene

Consequence: 不影响编译，但可能误导 reviewer 以为 E2E 夹具缺失。

Remedy: 保持 `.gitignore` 排除或只提交 `e2e/` spec 与 fixture，不提交 `.auth/run-id.txt`。

---

## v6 → v7 核销对照

| PR v6 Finding | v7 状态 | 证据 |
|---------------|---------|------|
| 🔴 变更集分裂 staged/unstaged/?? | ✅ FIXED | 3 主题 commit 已入库 |
| 🔴 smoke sed 解析为空 | ✅ FIXED | `--check-modules` 11 modules OK |
| 🟡 LiteParse 三次 parse | ✅ FIXED | `ParsedPdfSnapshot` 单 pass |
| 🟡 `-D warnings` 漂移 | ✅ FIXED | workspace + smoke prebuild 零 warning |
| 🟡 EdgeParse/Mineru 旧命名 | ✅ 文档化/compat | 非合并 blocker |
| 🟡 技术债报告未重计分 | ✅ FIXED | M15 四维复测 |
| 🟢 featureFlag 绕过 transport | ✅ FIXED | `lib/http/request` |
| 🟢 Paddle image E2E 缺口 | ✅ FIXED | `paddle_image_smoke.rs` |

---

## M15 验证记录

```bash
RUSTFLAGS="-D warnings" cargo check --workspace          # OK
./avrag-rs/scripts/run-product-smoke-e2e.sh --check-modules  # OK
bash scripts/verify-legal-p0.sh                        # 40/40
pnpm vitest run                                        # 293 passed
cargo test -p transport-http --lib auth_legal          # 5 passed
cargo test --manifest-path desktop/src-tauri/Cargo.toml  # 13 passed
cargo test -p avrag-share --tests                      # 14 passed
```

---

## Summary

v7 把 PR v6 的合并 blocker 全部拆除：变更集分层、smoke 守卫、warning 门禁、legal 合规与测试补强。当前 master 具备 PR 就绪态；合并前可选补跑全量 product smoke。

---

*报告生成：2026-06-13 · Brooks-Lint PR Review v7 · v6 已归档至 `docs/archive/brooks-pr-review-2026-06-13-v6.md`*
