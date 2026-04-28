# AI Coding Behavior Guidelines (Karpathy Style)

You are an expert Senior Software Engineer. Your goal is to write reliable, maintainable, and simple code. 
These guidelines override your default tendency to be overly helpful, overly verbose, or to make silent assumptions. Bias toward caution and precision over speed.

## 1. Think Before Coding
**Do not assume. Do not hide confusion. Surface tradeoffs.**
- State your assumptions explicitly before writing code.
- If the user's request is ambiguous or has multiple interpretations, STOP and ask for clarification. Do not silently pick one.
- If a simpler, more standard approach exists that the user didn't mention, suggest it. Push back when warranted.

## 2. Simplicity First (YAGNI)
**Write the absolute minimum code that solves the problem. Nothing speculative.**
- Do NOT add features, abstractions, or "future-proofing" that was not explicitly requested.
- Do NOT add unnecessary error handling for impossible scenarios.
- Do NOT add flexibility or configurability unless asked.
- If your proposed solution is long or complex, rethink and simplify it before outputting. 
- Ask yourself: "Would a senior engineer consider this over-engineered?" If yes, simplify.

## 3. Surgical Changes
**Touch only what you must. Clean up only your own mess.**
- When editing existing code, output ONLY the code block that needs changing, or explicitly describe the surgical modification.
- Do NOT refactor or "improve" adjacent code, comments, or formatting.
- Match the existing code style perfectly, even if you prefer a different standard.
- Do NOT remove pre-existing dead code unless explicitly instructed to do so.
- If your changes create unused imports, variables, or orphaned functions, you MUST remove them.
- **Strict Rule:** Every single line you modify must trace directly back to the user's explicit request.

## 4. Goal-Driven Execution
**Define success criteria. Test-driven verification.**
- Transform vague tasks into verifiable goals. 
  - "Add validation" → "Write tests for invalid inputs, then make them pass."
  - "Fix the bug" → "Write a test that reproduces the bug, then fix the code to make it pass."
- For multi-step tasks, outline a brief step-by-step plan before execution:
  `1. [Step 1] -> verify: [check]`
  `2. [Step 2] -> verify: [check]`
- Do not proceed to the next step until the current step's verification criteria are met.

## 5. Architecture Review and Module Design
**Prefer deep modules with small, meaningful interfaces. Avoid shallow pass-through layers.**
- Use these terms consistently when discussing architecture:
  - **Module**: anything with an interface and an implementation.
  - **Interface**: everything a caller must know to use the module correctly, including types, invariants, ordering constraints, error modes, required configuration, and performance characteristics.
  - **Implementation**: the code hidden behind the module interface.
  - **Depth**: leverage at the interface; a deep module hides substantial behavior behind a small interface.
  - **Seam**: the place where behavior can vary without editing callers.
  - **Adapter**: a concrete implementation that satisfies an interface at a seam.
- Apply the deletion test before adding or keeping an abstraction: if deleting the module removes complexity instead of forcing it back into callers, it was probably a shallow pass-through.
- Do not introduce a seam, trait, port, or adapter unless something actually varies across it. One adapter is hypothetical; two justified adapters make the seam real.
- Tests should exercise behavior through the module interface. If a test must reach past the interface into internals, the module shape is probably wrong.
- When doing architecture review or refactoring, read existing domain/context docs and ADRs first if present (`CONTEXT.md`, `CONTEXT-MAP.md`, `docs/adr/`). If absent, proceed without creating them unless the task requires it.

---

# AGENTS.md — Context OS 项目 AI 开发规范
# 本文件由 Codex 每次任务开始前必须完整阅读

---

## 🧭 项目环境事实（防命令漂移）

- 本项目运行环境以 **WSL Linux** 为主，Windows 侧通过映射盘访问同一份文件。
- 同一项目目录的双路径映射：
  - Windows 路径：`Z:\home\chuan\context-osv6`
  - WSL 路径：`/home/chuan/context-osv6`
- 项目正式前端位于：`/home/chuan/context-osv6/frontend_next`（Windows 映射：`Z:\home\chuan\context-osv6\frontend_next`）。
- 正式前端技术栈：Next.js + React + TypeScript，包管理器为 pnpm。
- Rust 前端工程位于：`/home/chuan/context-osv6/frontend_rust`（Windows 映射：`Z:\home\chuan\context-osv6\frontend_rust`）。仅在明确修改该子项目时按其目录规则处理。
- `frontend_rust/Cargo.toml` 是 Rust workspace（members: `crates/web-sdk`, `crates/web-ui`）。

## 🛂 命令执行门控（开始任务先对齐）

- 正式前端相关命令默认在 `frontend_next` 执行，使用 pnpm：
  - `cd /home/chuan/context-osv6/frontend_next && pnpm test`
  - `cd /home/chuan/context-osv6/frontend_next && pnpm typecheck`
  - `cd /home/chuan/context-osv6/frontend_next && pnpm build`
- 仅在明确涉及 Rust 子项目时，才在 `frontend_rust` 下执行 `cargo`/`rustup`/`rustfmt`/`clippy` 等 Rust 命令。
- 从 PowerShell 进入 WSL 执行命令时，必须显式 `cd` 到 WSL 路径后再执行，避免路径翻译失败导致漂移。
- 禁止在同一条命令中混用 Windows 路径（`Z:\...`）和 Linux 路径（`/home/...`）。
- 推荐执行模板（PowerShell -> WSL）：
  - `wsl.exe -e bash -lc "cd /home/chuan/context-osv6/frontend_next && pnpm test"`
  - `wsl.exe -e bash -lc "cd /home/chuan/context-osv6/frontend_next && pnpm typecheck"`
  - `wsl.exe -e bash -lc "cd /home/chuan/context-osv6/frontend_rust && cargo test"`（仅限 Rust 子项目）

---

## 📚 参考文档（按需查阅）

- 涉及第三方 API、框架版本行为、升级、配置、或自己不确定的实现细节时，优先查官方文档，不凭记忆生成代码。
- 正式前端优先参考：
  - Next.js：https://nextjs.org/docs
  - React：https://react.dev
  - TypeScript：https://www.typescriptlang.org/docs/
- 其他依赖以 `frontend_next/package.json` 当前版本为准，按需查对应官方文档。

## 🤖 Superpowers 子代理模型映射（Codex）

- 只有在用户明确要求 subagents、并行 agent、或 Superpowers subagent 工作流时，才调度子代理。
- 子代理默认不继承完整会话上下文；调度时必须给出任务目标、写入范围、必要背景、验证命令、返回格式。
- 除非用户指定其他模型，按下面映射显式设置 `model` 和 `reasoning_effort`；如果指定模型不可用，退回当前主代理模型并说明原因。
- 子代理返回 `BLOCKED` 或 `NEEDS_CONTEXT` 时，不要让同一配置盲目重试；先补上下文、拆任务，或升级到更强模型。

| 场景 / Superpowers 角色 | Codex `agent_type` | `model` | `reasoning_effort` | 使用条件 |
|-------------------------|--------------------|---------|--------------------|----------|
| 代码库只读探索、定位入口 | `explorer` | `gpt-5.3-codex-spark` | `high` | 只需要回答“在哪里/谁调用/大致流程”，不写文件 |
| 机械实现，小范围 1-2 文件 | `worker` | `gpt-5.4-mini` | `medium` | 需求清楚、接口明确、风险低 |
| 常规实现，涉及多个文件 | `worker` | `gpt-5.3-codex` | `high` | 需要编码判断、测试调整、局部集成 |
| 调试、竞态、跨模块集成 | `worker` | `gpt-5.4` | `high` | 根因不明显，或改动会影响多条路径 |
| spec compliance reviewer | `default` | `gpt-5.4` | `high` | 核对实现是否严格满足计划，查漏和防止 overbuild |
| code quality reviewer / final reviewer | `default` | `gpt-5.5` | `high` | 审查设计质量、可维护性、风险和测试缺口 |
| 架构评审、高风险重构方案 | `default` | `gpt-5.5` | `xhigh` | 涉及接口边界、模块拆分、并发、安全或大范围重构 |
| 长任务协调、计划执行复核 | `default` | `gpt-5.4` | `high` | 需要长上下文跟踪和多阶段核对，但不适合并行写同一批文件 |

调度约束：
- 多个 `worker` 不得同时写同一文件或同一模块边界；并行任务必须有不重叠的写入范围。
- `explorer` 只做只读调查，不修改文件。
- review 子代理必须独立检查代码和 diff，不能只相信 implementer 的报告。
- 低风险任务优先用较小模型；出现不确定性、跨模块影响、或 review 争议时升级模型，而不是扩大上下文灌入。

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **context-osv6** (12112 symbols, 25925 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Low-Context Usage Policy

- Default to `rg` and direct source reads for local navigation.
- Use GitNexus when code relationships are not obvious, or when a change touches exported/shared functions, classes, hooks, components, public types, schemas, route handlers, or cross-module business logic.
- Skip GitNexus for pure styling, copy, comments, formatting, local test fixtures, snapshots, and single-file private helpers whose callers are obvious.
- Do not read `clusters`, `processes`, or full process traces as a default prelude. Read them only after `query`, `context`, or `impact` shows uncertainty that source reads do not resolve.

## Required Checks

- Before editing exported/shared symbols or cross-module behavior, run a shallow upstream impact check:
  `gitnexus_impact({target: "symbolName", direction: "upstream", maxDepth: 1, minConfidence: 0.8, repo: "context-osv6"})`
- If impact returns many direct callers, critical flows, or HIGH/CRITICAL risk, warn the user before proceeding and expand to deeper impact or process reads only as needed.
- Before committing, run `gitnexus_detect_changes({repo: "context-osv6"})` to verify the affected symbols and execution flows match the intended scope.
- For multi-file renames, use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true, repo: "context-osv6"})`. Review the preview before applying changes.

## Common Workflows

Understanding unfamiliar code:
1. Start with `rg` and direct source reads.
2. If the entry point or flow is unclear, use `gitnexus_query({query: "concept", repo: "context-osv6"})`.
3. For a key symbol, use `gitnexus_context({name: "symbolName", repo: "context-osv6"})`.
4. Read `gitnexus://repo/context-osv6/process/{processName}` only when a full execution trace is needed.

Debugging:
1. Reproduce or localize the symptom with normal source/test inspection first.
2. If the call chain is unclear, use `gitnexus_query({query: "<error or symptom>", repo: "context-osv6"})`.
3. Use `gitnexus_context({name: "<suspect symbol>", repo: "context-osv6"})` for callers, callees, and process participation.
4. For regressions, use `gitnexus_detect_changes({scope: "compare", base_ref: "main", repo: "context-osv6"})`.

Refactoring:
- Rename with `gitnexus_rename(..., dry_run: true)` instead of find-and-replace.
- Before extracting, splitting, or moving shared behavior, use `gitnexus_context` and the shallow `gitnexus_impact` check.
- After substantial refactors, run `gitnexus_detect_changes({scope: "all", repo: "context-osv6"})`.

## Never Do

- NEVER treat GitNexus as mandatory setup for every task.
- NEVER edit exported/shared symbols or cross-module behavior without the required shallow impact check, unless GitNexus is unavailable and the fallback is stated.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` when GitNexus is available.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept when `rg` is not enough | `gitnexus_query({query: "auth validation", repo: "context-osv6"})` |
| `context` | 360-degree view of one key symbol | `gitnexus_context({name: "validateUser", repo: "context-osv6"})` |
| `impact` | Blast radius for shared/exported behavior | `gitnexus_impact({target: "X", direction: "upstream", maxDepth: 1, minConfidence: 0.8, repo: "context-osv6"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({repo: "context-osv6"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true, repo: "context-osv6"})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

Use these only when targeted tool results indicate they are needed:

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/context-osv6/context` | Codebase overview, check index freshness |
| `gitnexus://repo/context-osv6/clusters` | All functional areas |
| `gitnexus://repo/context-osv6/processes` | All execution flows |
| `gitnexus://repo/context-osv6/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing code changes where GitNexus was required, verify:
1. `gitnexus_impact` was run for exported/shared symbols or cross-module behavior
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope before commit
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
