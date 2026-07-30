# OpenCodeReview 审查报告 — context-osv6 工作区变更

> 审查工具：[alibaba/open-code-review](https://github.com/alibaba/open-code-review) `ocr` v1.8.1
> 审查时间：2026-07-31
> LLM：DeepSeek `deepseek-v4-pro`（provider: deepseek，OpenAI 兼容）
> 审查对象：context-osv6 当前工作区未提交变更（master 分支）

## 执行统计

| 项 | 值 |
|----|----|
| 待审文件（preview） | 96 文件变更，+3663 / -1658 |
| 实际审查 | **42 文件**（Rust `.rs` + YAML） |
| 排除 | 54 文件，全部 `.md`（`unsupported_ext`） |
| 审查意见 | **31 条**（high 1 / medium 15 / low 15） |
| Token 消耗 | ~4,733,490（输入 4.48M / 输出 257K，cache 读 3.86M） |
| 耗时 | 10m33s |

## 覆盖范围说明（重要）

- OCR 默认按扩展名把 `.md` 当文档**排除**。本次被排除的 54 个 `.md` 里包含 `avrag-rs/prompts/**/*.md`——按本项目 **prompts-in-md 规则，这些是产品代码（LLM 指令）**，未被本次审查覆盖。如需审查 prompt 文案，需单独处理（OCR 默认不支持 .md 规则）。
- 已知工具瑕疵：OCR 的 codebase 搜索在解析模块路径时两次失败（`exit_policy.rs` 漏 `policy/`、`pipeline_tests.rs` 漏 `avrag-rs/`），相应意见基于 diff 生成、缺少部分上下文，非致命。

## 发现总览（按严重度）

### 🔴 HIGH（1）— 真实功能 Bug

1. **`agent-loop/src/helpers/selected.rs:14-20` — `graph_retrieval` 缺失于 `ALIASED_TOOLS`**
   SELECTED 别名解析会对所有 graph 检索结果静默跳过。镜像实现 `orchestrator/selected.rs` 里是包含 `graph_retrieval` 的。后果：任何映射到 graph 检索块的 `SELECTED: #n` 别名解析失败被丢弃。**修复：在 `ALIASED_TOOLS` 补 `"graph_retrieval"`。**

### 🟠 MEDIUM（15）

**Bug 类（6）**
- `tool_registry.rs:35-74` — `doc_profile`/`doc_summary`/`doc_scan` 同时在 `CODEGEN_SDK_METHOD_NAMES` 与 `SAC_SUPERSEDED_NATIVE_TOOLS`，dispatch 先查前者导致它们得到泛化 `not_a_native_tool` 错误，而非更精确的 `sac_sdk_only` + SaC hint，可能误导模型。
- `progress/mod.rs:288` — `user_profile` 标签不一致：`bridge_method_progress`="读取用户画像" vs `labels.rs`="回忆相关上下文"。
- `selected.rs:71-79` — `graph_augment` 遥测守卫当前是死代码（依赖上面的 high bug 修复后才可达且正确）。
- `prompt_assets.rs:120-126` — `degraded_no_evidence_answer` 未处理合法 mode id `"rag+search"`，会落到 default（纯 chat 文案），与 `contract_violation_fallback` 有专门的 dual 分支不一致。
- `helpers/mod.rs` — 死代码 if 分支（graph_retrieval 已被前置过滤跳过）。

**Maintainability / 安全（重点 3）**
- ⚠️ **`deps.rs:128-145` — 死代码 + 潜在安全风险**：`execute_codegen_bridged` 零调用者，构造**空 `HashSet` 作 `sdk_allowed`**，因 `method_allowed` 的 `is_empty()=>true` 会**无条件放行所有 SDK 方法**（web/fetch/history/user_profile/save/load），且每次 `SessionFs::new()` 破坏跨调用持久化。建议 `#[cfg(test)]` / `#[doc(hidden)]` / 删除。
- ⚠️ **`tool_registry.rs:106-135` — 违反 prompts-in-md 硬规则**：`reject_sac_superseded_native_tool` 在 Rust 里硬编码 LLM-facing 指令文案（"Output one `<code ...>` block..."）。按项目非协商规则应移到 `prompts/loop/*.md` 用 `include_str!` + 占位符。
- `session_fs.rs:45-52` — poisoned `Mutex` 用 `unwrap_or_else(into_inner)` 静默恢复，可能传播损坏的 session 数据，建议至少 `tracing::warn!` 记录。

**死代码清理债（4）**
- `exit_policy.rs:45` — `RunFallbackThenCheck` 变体改动后永不返回，下游 `run_synthesis.rs:85` / `run_fallback.rs` 不可达。
- `exit_policy.rs:285` — `decide_post_loop` 无条件返回 `EnterSynthesis`，`DegradedNoEvidence` 路径不可达。
- `external_agent_guide.rs:49-51` — `load_mode_config("search")` 做 IO 但结果被 `let _ =` 丢弃，静默吞错误。
- `mode_assemble.rs:153` — `merge_tool_pool` 调用点全被移除。

**Test 断言弱化（2）**
- `assembler.rs:280-285` — `contains("dense")` 过宽（匹配 "condense" 等），让 codegen skill 文档校验形同虚设。
- `iteration/tests.rs:690-695` — `reason.is_some()` fallback 让退出原因断言对任意值通过。
- `pipeline_tests.rs:244-249` — 断言从检查 `"### User question"` 弱化为 `!answer.is_empty()`。

### 🟡 LOW（15）
小问题：test 死分支、命名误导、`Vec::contains` O(n²)（列表极小，影响可忽略）、stale doc 注释、`normalize_key` 未规范化 `./` 与 `//`、`subst` 模板替换脆弱等。详见下方英文原文。

## 总体评价

本次变更（agent-loop 重构 + Search-as-Code SDK 化）方向正确，未发现 Rust 内存安全 / 所有权层面的严重错误（编译器兜底）。主要问题集中在：① 一两处真实路由 / 别名 bug（high 的 graph_retrieval、medium 的 doc_* 重复、rag+search 漏路由）；② 重构遗留较多死代码（exit_policy 两条分支、external_agent_guide、mode_assemble、rag-core bridge 两个 arm）；③ 一处 prompts-in-md 规则违规；④ 一处潜在安全风险的死代码方法。建议优先修 high + 两处 ⚠️ medium。

---

## 英文原文（OCR 输出，31 条）

─── avrag-rs/crates/agent-loop/src/react_loop/assembler.rs:280-285 ───
[test · medium] The fallback `ctx.system_content.contains("dense")` is overly broad — it matches any
occurrence of the substring "dense" (including "condense", "dense_xyz", etc.), which can silently
pass even when the codegen skill documentation is broken or absent. This contrasts with the
equivalent assertion in `rag_round_one_re_injects_codegen_skill` (line 337–339), which correctly
uses only `"client.dense"` and `"dense(query)"`. Remove the bare `"dense"` fallback to avoid false
positives.

          assert!(
              ctx.system_content.contains("client.dense")
-                 || ctx.system_content.contains("dense(query)")
-                 || ctx.system_content.contains("dense"),
+                 || ctx.system_content.contains("dense(query)"),
              "codegen skill should document dense retrieval"
          );


─── avrag-rs/crates/agent-loop/src/react_loop/assembler.rs:280-285 ───
[style · low] The two assertions that verify codegen SDK signatures use inconsistent patterns: the
round-0 test (line 280) has a broad `"dense"` fallback, while the round-1 test (line 337) only
checks `"client.dense"` and `"dense(query)"`. They should use the same pattern for consistency,
since both are verifying the same class of behavior (codegen skill presence in system content).

          assert!(
              ctx.system_content.contains("client.dense")
-                 || ctx.system_content.contains("dense(query)")
-                 || ctx.system_content.contains("dense"),
+                 || ctx.system_content.contains("dense(query)"),
              "codegen skill should document dense retrieval"
          );


─── avrag-rs/crates/agent-loop/src/helpers/mod.rs:0-0 ───
[bug · medium] Dead code: `graph_retrieval` is not in `ALIASED_TOOLS`, so the prior filter at line
68 (`!ALIASED_TOOLS.contains(&tr.tool.as_str())`) already skips all `graph_retrieval` results. This
block will never execute. Either `graph_retrieval` should be added to `ALIASED_TOOLS`, or this `if`
branch should be removed.



─── avrag-rs/crates/agent-loop/src/progress/mod.rs:288-288 ───
[bug · medium] The label for `"user_profile"` is inconsistent between `bridge_method_progress`
("读取用户画像") and `product_action_for_bridge_method` in `labels.rs` ("回忆相关上下文"). Both functions map
bridge method names to product labels and should agree on the same method. Pick one label and use it
in both places.

-         "user_profile" => Some((ProgressKind::Memory, "读取用户画像")),
+         "user_profile" => Some((ProgressKind::Memory, "回忆相关上下文")),


─── avrag-rs/crates/agent-loop/src/react_loop/iteration/tests.rs:690-695 ───
[test · medium] The `reason.is_some()` fallback makes this assertion effectively pass for any exit
reason, not just the two expected outcomes (`compile_feedback` or `direct_content`). Per the
production code in `content_dispatch.rs`, the worker compile path can only produce those two reasons
— all others (e.g. a regression reintroducing `content_blocked_no_evidence`) would be bugs. Tighten
to only accept the two valid reasons.

      assert!(
          reason == Some("compile_feedback")
-             || reason == Some("direct_content")
-             || reason.is_some(),
+             || reason == Some("direct_content"),
          "unexpected reason {reason:?}"
      );


─── avrag-rs/crates/agent-loop/src/react_loop/iteration/tests.rs:413-424 ───
[test · low] The prompt asset at `prompts/loop/blocks-skipped.nudge.md` reads: "其余 {n_skipped}
个未执行。每轮仅第一块进入沙箱". Three OR branches here can never match the actual prompt text: (1) `"其余 2 个未被执行"`
— prompt uses "未执行" not "未被执行"; (2) `"跳过了 2"` — prompt does not contain "跳过"; (3) `"每轮只"` — prompt
uses "每轮仅" not "每轮只". These dead branches are harmless noise. Consider removing them or replacing
with the exact prompt substring.

      assert!(
-         observation.contains("其余 2 个未执行")
-             || observation.contains("其余 2 个未被执行")
-             || observation.contains("跳过了 2"),
+         observation.contains("其余 2 个未执行"),
          "{observation}"
      );
      assert!(
          observation.contains("仅第一块进入沙箱")
-             || observation.contains("每轮仅第一块")
-             || observation.contains("每轮只"),
+             || observation.contains("每轮仅第一块"),
          "{observation}"
      );


─── avrag-rs/crates/agent-tools/src/capability/api.rs:87-90 ───
[documentation · low] Stale doc comment: the old behavior merged `auto_fallback.tool_id` into the
disclosure; the new behavior (A1/D6) deliberately excludes it. Update this comment to reflect that
`auto_fallback` is host-internal and not disclosed to the LLM.

-     /// RAG (and others) may keep `tool_pool` empty for on-demand skill disclosure
-     /// while still auto-invoking a retrieval tool — include it in product capability list.
+     /// RAG (and others) may keep `tool_pool` empty for on-demand skill disclosure.
+     /// `auto_fallback.tool_id` is host-internal (dense/web after budget) and
+     /// must **not** appear in the LLM-facing capabilities API.
      #[serde(default)]
      auto_fallback: Option<ModeYamlAutoFallback>,


─── avrag-rs/crates/agent-tools/src/capability/api.rs:143-147 ───
[maintainability · low] After this change, `load_mode_disclosed_tools` and `load_mode_tool_pool`
have identical implementations. `load_mode_disclosed_tools` is only called once (in
`product_mode_tool_pool_union`). Consider either: (a) deleting `load_mode_disclosed_tools` and
calling `load_mode_tool_pool` directly from `product_mode_tool_pool_union`, or (b) adding a brief
comment explaining why the two functions exist separately despite being identical (e.g., reserving
for future divergence between LLM-facing and schema-facing pools).

  fn load_mode_disclosed_tools(file_stem: &str) -> Vec<String> {
-     parse_mode_yaml(file_stem)
-         .map(|m| m.tool_pool)
-         .unwrap_or_default()
+     // Currently identical to load_mode_tool_pool; kept separate to allow
+     // future divergence between LLM-facing disclosure and schema tool_pool.
+     load_mode_tool_pool(file_stem)
  }


─── avrag-rs/crates/agent-loop/src/react_loop/policy/exit_policy.rs:45-45 ───
[maintainability · medium] `RunFallbackThenCheck` is never returned by `decide_synthesis_gate` after
this change. The match arm in `run_synthesis.rs:85` calling
`trigger_auto_fallback_and_check_degraded` is unreachable, which in turn makes
`finish_degraded_no_evidence_run` unreachable. Consider either removing the `RunFallbackThenCheck`
variant (and all dependent dead code in `run_synthesis.rs` / `run_fallback.rs`) or documenting why
it is intentionally retained for future re-enablement.

-     RunFallbackThenCheck,
+     // RunFallbackThenCheck — retired: host no longer forces fallback for
+     // missing evidence (skill-owned gating). Remove when call-site cleanup completes.


─── avrag-rs/crates/agent-loop/src/react_loop/policy/exit_policy.rs:285-286 ───
[maintainability · medium] `decide_post_loop` unconditionally returns `EnterSynthesis`, so
`post_fallback_gate` (line 70) never returns `DegradedNoEvidence`. The guard
`post_fallback_gate(...) != PostLoopAction::DegradedNoEvidence` in `run_fallback.rs:49` is always
`true`, and the `finish_degraded_no_evidence_run` codepath is unreachable. Consider removing
`DegradedNoEvidence` from the enum and cleaning up `trigger_auto_fallback_and_check_degraded` if the
degraded path is permanently retired, or add a comment explaining the retention strategy.



─── avrag-rs/crates/agent-loop/src/react_loop/deps.rs:128-145 ───
[maintainability · medium] **Dead code with latent security risk.** `execute_codegen_bridged` (lines
128-145) is a `pub` method with zero callers anywhere in the workspace. It constructs an empty
`HashSet` as `sdk_allowed`, which — due to `method_allowed`'s `is_empty() => true` gate — would
allow all SDK methods (web, fetch, history, user_profile, save, load) unconditionally. It also
creates a fresh `SessionFs::new()` per invocation, silently breaking cross-invocation file
persistence.

If this method is intentionally kept as a test helper, annotate it `#[cfg(test)]` (or
`#[doc(hidden)]` with a clear safety comment). Otherwise, remove it to prevent accidental production
use.

+     #[doc(hidden)] // test / legacy convenience; prefer execute_codegen_bridged_with_session
      pub async fn execute_codegen_bridged(
          &self,
          code: &str,
          auth: &AuthContext,
          doc_scope: &[String],
          alias_counter: Arc<AtomicU64>,
      ) -> BridgedCodegenExec {
          self.execute_codegen_bridged_with_session(
              code,
              auth,
              doc_scope,
              alias_counter,
              None,
              Arc::new(super::session_fs::SessionFs::new()),
              Arc::new(HashSet::new()),
          )
          .await
      }


─── avrag-rs/crates/agent-loop/src/react_loop/deps.rs:305-316 ───
[maintainability · low] **Sequential Mutex locking creates a window where `extra_results` and
`extra_calls` are out of sync.** `record_extra` locks `extra_results`, pushes, releases, then locks
`extra_calls` and pushes. If `take_all_results()` were called between the two lock acquisitions
(e.g., in a future concurrent context), the two vectors would become desynchronized — one would
contain an entry the other lacks.

Currently safe because the bridge is used exclusively within a single code-interpreter execution,
but worth noting: consider locking both mutexes together (e.g., via a single
`Mutex<(Vec<ToolResult>, Vec<BridgeCallObs>)>`) to make the invariant structural.

-         self.extra_results
+         // Consider: single Mutex<(Vec<ToolResult>, Vec<BridgeCallObs>)> to
+         // keep results/calls synchronised under one lock acquisition.
+         {
+             let mut results = self.extra_results
              .lock()
-             .unwrap_or_else(|e| e.into_inner())
-             .push(result.clone());
-         self.extra_calls
+                 .unwrap_or_else(|e| e.into_inner());
+             let mut calls = self.extra_calls
              .lock()
-             .unwrap_or_else(|e| e.into_inner())
-             .push(BridgeCallObs {
+                 .unwrap_or_else(|e| e.into_inner());
+             results.push(result.clone());
+             calls.push(BridgeCallObs {
                  method: method.to_string(),
                  query,
                  result,
              });
+         }


─── avrag-rs/crates/app-chat/src/external_agent_guide.rs:49-51 ───
[maintainability · medium] Dead code: `load_mode_config("search")` performs file I/O (reads
`modes/search.yaml`, parses YAML, validates) and returns a `Result`. Discarding the result with `let
_ =` silently swallows any I/O error or parse failure. The original code used this to populate
`tool_schemas`, but now `tool_schemas` is hardcoded to `Vec::new()`. These two lines should be
removed entirely.



─── avrag-rs/crates/app-chat/src/external_agent_guide.rs:3-4 ───
[maintainability · low] Once the dead code is removed, these two imports will become unused and
should also be removed to keep the module clean.



─── avrag-rs/crates/agent-tools/src/tool_registry.rs:106-135 ───
[maintainability · medium] The `reject_sac_superseded_native_tool` function hard-codes LLM-facing
instructional prose (e.g. "Output one `<code language="python">` block using the sandbox client").
Per the project rule, all LLM-facing instructional text must reside in `prompts/loop/*.md` and be
loaded via `include_str!`, following the pattern established in `prompt_assets.rs`. Hard-coding such
text in Rust makes it harder to maintain, translate, or version independently from logic.

Suggested fix: move the rejection template to a prompt asset (e.g.
`prompts/loop/sac-superseded-rejection.tmpl.md`) and use `include_str!` with `{tool}` and
`{sac_hint}` placeholder substitution, mirroring how `budget_exhausted_carryover` works in
`prompt_assets.rs`.

  /// Reject LLM native tool_calls for tools that moved into the SaC sandbox SDK (A1).
  pub fn reject_sac_superseded_native_tool(tool_name: &str) -> ToolResult {
      let sac_hint = match tool_name {
          "dense_retrieval" => "await client.dense(query)",
          "lexical_retrieval" => "await client.lexical(query)",
          "web_search" => "await client.web(query)",
          "web_fetch" => "await client.fetch(url)",
          "doc_summary" => "await client.doc_summary(...)",
          "doc_profile" => "await client.doc_profile(...)",
          "doc_grep" | "doc_scan" => "await client.grep(pattern, ...)",
          "doc_read_lines" => "await client.grep(..., context=N)  # read_lines removed",
          "graph_retrieval" => "await client.lexical(query)  # graph is bound to lexical",
          _ => "await client.<method>(...)  # see codegen/search skill",
      };
      ToolResult {
          tool: tool_name.to_string(),
          version: "1.0".to_string(),
          status: ToolStatus::Error,
          data: Some(serde_json::json!({
              "error": "sac_sdk_only",
              "tool": tool_name,
-             "hint": format!(
-                 "`{tool_name}` is not available as a native function call (Search-as-Code). \
-                  Output one `<code language=\"python\">` block using the sandbox client, e.g. \
-                  `{sac_hint}`."
-             ),
+             "hint": sac_superseded_rejection_hint(tool_name, sac_hint),
          })),
          trace: None,
      }
  }


─── avrag-rs/crates/agent-tools/src/tool_registry.rs:35-74 ───
[bug · medium] `doc_profile`, `doc_summary`, and `doc_scan` appear in both
`CODEGEN_SDK_METHOD_NAMES` and `SAC_SUPERSEDED_NATIVE_TOOLS`. Because `dispatch_tool` checks the
codegen list first (line 144), these tools receive the generic `not_a_native_tool` error (suggesting
`await client.doc_profile(...)`) instead of the more precise `sac_sdk_only` error which includes a
concrete SaC hint. This inconsistency may confuse the model — it gets told the tool "is not a native
tool" when it actually was a native tool that has been migrated.

Consider either:
- Removing the three overlapping names from `CODEGEN_SDK_METHOD_NAMES` so they fall through to the
SaC rejection with the better hint, or
- Swapping the check order so `is_sac_superseded_native_tool` is evaluated first for former native
tools.



─── avrag-rs/crates/agent-tools/src/tool_registry.rs:409-420 ───
[test · low] Only `dense_retrieval` and `web_search` are tested for the new
`reject_sac_superseded_native_tool` / `is_sac_superseded_native_tool` functions. The remaining 10
tool names in `SAC_SUPERSEDED_NATIVE_TOOLS` (e.g. `lexical_retrieval`, `graph_retrieval`,
`index_lookup`, `doc_metadata`, `doc_scan`, `doc_grep`, `doc_read_lines`, `web_fetch`,
`doc_summary`, `doc_profile`) have no unit-test coverage for the SaC rejection path. If the list is
accidentally trimmed or a name is misspelled, the regression would go undetected.

Consider adding a parameterized test that iterates over all entries in `SAC_SUPERSEDED_NATIVE_TOOLS`
and asserts they are recognized by `is_sac_superseded_native_tool` and produce `sac_sdk_only`
errors.



─── avrag-rs/crates/app-chat/src/chat/pipeline_tests.rs:244-249 ───
[test · medium] The answer-content assertion was weakened from checking a specific marker (`"###
User question"`) to only `!answer.is_empty()`. Since `PipelineEchoAgent` simply echoes
`request.query` (which originates from the user query `"test"`), a more precise assertion like
`execution.response.answer.contains("test")` would verify that the agent processed the actual user
question rather than just producing any non-empty string. The current assertion would pass even if
the agent returned a generic placeholder, hiding regressions in query construction or agent
dispatch.

          // Mock agent echoes the user query directly (no synthesize handoff pack).
          assert!(
-             !execution.response.answer.is_empty(),
-             "single-agent answer expected: {}",
+             execution.response.answer.contains("test"),
+             "single-agent answer must contain user question: {}",
              execution.response.answer
          );


─── avrag-rs/crates/app-chat/src/chat/pipeline_tests.rs:506-506 ───
[maintainability · low] Test function name
`dispatch_phase_loads_orchestrator_and_capability_prompts` is now misleading — the test verifies
that orchestrator metadata is absent and that capability manuals (not orchestrator prompts) are
loaded. Consider renaming to reflect the single-agent semantics, e.g.,
`dispatch_phase_loads_capability_manuals_only`.



─── avrag-rs/crates/agent-loop/src/helpers/selected.rs:14-20 ───
[bug · high] **`graph_retrieval` is missing from `ALIASED_TOOLS`, making SELECTED alias resolution
silently skip all graph retrieval results.**

The mirrored implementation in `orchestrator/selected.rs` includes `"graph_retrieval"` in
`ALIASED_TOOLS`. Without it, `alias_chunk_ids_in_order` will never enumerate chunks from
`graph_retrieval` results, so any `SELECTED: #n` alias that maps to a graph-retrieved chunk will
fail to resolve (logged as a warning and dropped).

Additionally, the `graph_retrieval` + `degrade_reason == "graph_augment"` guard on lines 71–78 is
unreachable dead code — the enclosing `!ALIASED_TOOLS.contains(...)` guard on line 62 already skips
every `graph_retrieval` result.

  const ALIASED_TOOLS: &[&str] = &[
      "dense_retrieval",
      "lexical_retrieval",
+     "graph_retrieval",
      "index_lookup",
      "doc_grep",
      "doc_read_lines",
  ];


─── avrag-rs/crates/agent-loop/src/helpers/selected.rs:71-79 ───
[bug · medium] The `graph_augment` telemetry guard on lines 71–78 is currently dead code because
`graph_retrieval` is not in `ALIASED_TOOLS`. If `graph_retrieval` is added back to the list (as
recommended above), this guard becomes reachable and is **correct** — it filters out force-augment
side-car results that should not contribute to the alias namespace.

However, note that the `trace` field on `ToolResult` is `#[serde(skip)]` in many contexts and may be
`None` when results are deserialized across boundaries. Verify that `trace` is reliably populated on
the code path that feeds `tool_results` into this function.

+         // graph_augment side-car results carry telemetry but no meaningful
+         // chunks for SELECTED aliases; filter them out.
          if tr.tool == "graph_retrieval"
              && tr
                  .trace
                  .as_ref()
                  .and_then(|t| t.degrade_reason.as_deref())
                  == Some("graph_augment")
          {
              continue;
          }


─── avrag-rs/crates/agent-loop/src/helpers/selected.rs:119-121 ───
[performance · low] `resolve_selected_chunk_ids` uses `out.contains(&id)` (O(n) linear scan per
insertion), making the function O(n²) in the number of resolved aliases. While typical alias counts
are small (< 20), using a `HashSet` for the seen set would be clearer and avoids quadratic behavior
if the alias list grows.

The mirrored `orchestrator/selected.rs` has the same pattern (`out.iter().any(|c| c.chunk_id ==
chunk.chunk_id)`), so this is a design consistency question rather than a new defect.

+         // Consider using a HashSet<String> alongside the Vec for O(1) dedup.
          if !out.contains(&id) {
              out.push(id);
          }


─── avrag-rs/crates/app-chat/src/mode_assemble.rs:153-153 ───
[maintainability · medium] Dead code: `merge_tool_pool` is no longer called anywhere after both call
sites were removed in this diff. It should be removed or annotated with `#[allow(dead_code)]` if
intentionally kept as a reference. Leaving it defined misleads maintainers into thinking it is still
used for merging mode tool pools.

+ #[allow(dead_code)]
  fn merge_tool_pool(dst: &mut Vec<String>, src: &[String]) {


─── avrag-rs/crates/agent-loop/src/react_loop/sdk_gate.rs:25-40 ───
[performance · low] `Vec::contains` is O(n) and is called inside a loop, making the deduplication
O(n²). Since the list is tiny (~16 items) and the function is only called once at config time
(mode_assemble.rs:130), the practical impact is negligible. However, using a `HashSet` for
deduplication would be more intention-revealing and avoid the anti-pattern:

-     let mut out: Vec<&'static str> = BASE_PRIMITIVES.to_vec();
+     let mut out: HashSet<&'static str> = BASE_PRIMITIVES.iter().copied().collect();
      if rag {
-         for p in RAG_PRIMITIVES {
-             if !out.contains(p) {
-                 out.push(p);
-             }
-         }
+         out.extend(RAG_PRIMITIVES);
      }
      if search {
-         for p in SEARCH_PRIMITIVES {
-             if !out.contains(p) {
-                 out.push(p);
-             }
-         }
+         out.extend(SEARCH_PRIMITIVES);
      }
-     out
+     out.into_iter().collect()


─── avrag-rs/crates/rag-core/src/runtime/bridge.rs:79-86 ───
[maintainability · low] Dead code: the `"web"` and `"fetch"` arms in `extract_query` are
unreachable. `method_to_tool_call` rejects these methods with an error (line 283), so `call()`
returns early at line 405 before `extract_query` is invoked at line 421. If RuntimeBridge is never
intended to handle web/fetch, these arms should be removed to avoid misleading future maintainers.
If they are intentionally left as forward-compatibility stubs, add a comment explaining the intent.

-             "dense" | "dense_search" | "lexical" | "lexical_search" | "web" => args
+             "dense" | "dense_search" | "lexical" | "lexical_search" => args
                  .get("query")
-                 .and_then(|v| v.as_str())
-                 .map(str::to_owned),
-             "fetch" => args
-                 .get("url")
                  .and_then(|v| v.as_str())
                  .map(str::to_owned),


─── avrag-rs/crates/rag-core/src/runtime/bridge.rs:341-343 ───
[maintainability · low] Dead code: the `"web_search" | "web_fetch" | "conversation_history_load" |
"user_profile_load"` arms in `tool_result_to_bridge_data` are unreachable. `method_to_tool_call`
rejects `"web"`, `"fetch"`, `"history"`, and `"user_profile"` with an error before any tool dispatch
occurs, so RuntimeBridge never produces `ToolResult` values with these tool names. The composite
`SacHostBridge` in agent-loop handles these methods via its own `record_extra` path instead. These
arms should be removed to prevent confusion about what RuntimeBridge actually handles.



─── avrag-rs/crates/agent-loop/src/react_loop/prompt_assets.rs:120-126 ───
[bug · medium] `degraded_no_evidence_answer` does not handle the "rag+search" mode, which is a
legitimate mode ID produced by `CapabilitySet::agent_type_label()` and set as `config.id` in
`mode_assemble.rs:123`. When both RAG and Search capabilities are active, the agent will fall
through to the default (`_`) arm and use `degraded-no-evidence-default.md` — the same prompt as pure
chat. This is inconsistent with `contract_violation_fallback` (line 111), which has a dedicated
"rag+search" → `contract-violation-dual.md` arm. Add a "rag+search" arm or explicitly route it to
the rag variant.

  pub fn degraded_no_evidence_answer(mode_id: &str) -> &'static str {
      match mode_id {
-         "rag" => trim_body(loop_prompt!("degraded-no-evidence-rag.md")),
+         "rag" | "rag+search" => trim_body(loop_prompt!("degraded-no-evidence-rag.md")),
          "search" => trim_body(loop_prompt!("degraded-no-evidence-search.md")),
          _ => trim_body(loop_prompt!("degraded-no-evidence-default.md")),
      }
  }


─── avrag-rs/crates/agent-loop/src/react_loop/prompt_assets.rs:26-32 ───
[maintainability · low] The `subst` function performs naive global string replacement iterating over
`(key, value)` pairs. If the `body` parameter passed to `budget_exhausted_carryover` contains brace
patterns like `{body}` or `{tool}`, they will be unintentionally substituted. While the current call
site (`run_synthesis.rs:371`) passes truncated tool output that is unlikely to contain these exact
patterns, the approach is fragile. Consider using a template engine or escaping user-provided values
before substitution.



─── avrag-rs/crates/agent-loop/src/react_loop/prompt_assets.rs:12-20 ───
[maintainability · low] The `loop_prompt!` macro constructs the path as
`$CARGO_MANIFEST_DIR/../../prompts/loop/`, relying on a hard-coded relative relationship between the
crate directory and the prompts directory. If the `agent-loop` crate is moved or the prompts
directory is relocated, this will break at compile time via `include_str!`. Consider using a
workspace-level environment variable (set in `build.rs`) or a crate like `cargo-manifest-dir` to
make the prompt root configurable.



─── avrag-rs/crates/agent-loop/src/react_loop/session_fs.rs:45-52 ───
[maintainability · medium] `save`, `load`, and `len` recover from a poisoned `Mutex` via
`unwrap_or_else(|e| e.into_inner())`, silently continuing with the inner `HashMap` that may be in an
inconsistent state after a panic in another thread holding the same lock. While this follows a
codebase-wide pattern, in a long-running agent loop this could mask a prior panic and propagate
corrupted session data. Consider at least logging the poison event via `tracing::warn!` or
`log::warn!` before recovering, so that failures are observable in production telemetry.

      pub fn save(&self, path: &str, data: Value) -> Result<(), String> {
          let key = Self::normalize_key(path)?;
-         self.files
-             .lock()
-             .unwrap_or_else(|e| e.into_inner())
-             .insert(key, data);
+         let mut guard = self.files.lock().map_err(|e| {
+             tracing::warn!("SessionFs mutex poisoned; recovering with potentially stale data");
+             e.into_inner()
+         });
+         guard.insert(key, data);
          Ok(())
      }


─── avrag-rs/crates/agent-loop/src/react_loop/session_fs.rs:24-43 ───
[maintainability · low] `normalize_key` does not reject or canonicalize dot-only segments (`.` or
`./`), duplicate slashes (`foo//bar`), or trailing slashes (`foo/`). Since the store is keyed by
exact string match, callers using inconsistent formats (e.g., `save("./data.json", v)` then
`load("data.json")`) will silently miss each other. Consider stripping leading `./`, collapsing
consecutive `/`, and removing trailing `/` to normalize keys into a canonical form.

      pub fn normalize_key(path: &str) -> Result<String, String> {
-         let p = path.trim();
+         let mut p = path.trim().to_string();
          if p.is_empty() {
              return Err("path is required".into());
          }
          if p.contains('\0') {
              return Err("path must not contain NUL".into());
          }
          if p.starts_with('/') || p.starts_with('\\') {
              return Err("path must be relative (no leading slash)".into());
          }
          if p.contains("..") {
              return Err("path must not contain '..'".into());
          }
          // Reject Windows drive letters.
          if p.len() >= 2 && p.as_bytes()[1] == b':' {
              return Err("path must not be an absolute Windows path".into());
          }
-         Ok(p.to_string())
+         // Canonicalize: strip leading "./", collapse duplicate slashes, strip trailing slash.
+         while p.starts_with("./") {
+             p = p[2..].to_string();
+         }
+         while p.contains("//") {
+             p = p.replace("//", "/");
+         }
+         if p.len() > 1 {
+             p = p.trim_end_matches('/').to_string();
+         }
+         Ok(p)
      }

