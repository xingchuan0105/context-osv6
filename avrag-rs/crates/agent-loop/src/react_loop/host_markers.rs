//! 宿主观察标签备案制（印章备案制，D3）。
//!
//! 单一事实源：所有由宿主注入到模型上下文的观察标签都必须先在此登记，
//! 然后发射端引用常量、检测器从表派生。parity 测试扫描
//! `prompts/loop/*.md` 与发射端源码，未备案标签 → 测试红。
//!
//! 约定（AGENTS.md 硬规则）：宿主注入的观察标签必须先登记本模块再使用。
//!
//! `forbidden_in_final`：该标签出现在终答中即视为「仿造宿主观察外壳」
//! （`answer_contract::contains_host_observation_shell` 的检测集）。纯格式
//! 判定，非语义 keyword bar（AGENTS.md stop-decision）。

/// 一枚已备案的宿主观察标签。
pub struct HostMarker {
    /// 前缀匹配形态（如 `"<loop_budget"`、`"[retrieval_summary]"`）。
    /// 检测与发射端都以该字符串作字面前缀匹配，因此截断/改写闭合的
    /// 仿造变体仍能被命中。
    pub tag: &'static str,
    /// 该标签出现在终答中是否构成违规（检测器派生依据）。
    pub forbidden_in_final: bool,
    /// 发射端位置说明（人读，parity 测试按 `.rs` 或 `.md` 文件路径校验）。
    pub emitted_at: &'static str,
}

/// 宿主观察标签备案表（单一事实源）。
///
/// 新增标签 = 表内加一行；发射端改引用常量；检测器自动覆盖。
pub const HOST_OBSERVATION_MARKERS: &[HostMarker] = &[
    // --- 代码执行 / 沙箱 ---
    HostMarker {
        tag: "<code_execution_result>",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/synthesis-prose-repair.tmpl.md::教学引用的闭合形态",
    },
    HostMarker {
        tag: "<code_execution_result ",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/react_loop/iteration_codegen.rs::format_codegen_result_message",
    },
    HostMarker {
        tag: "[no_output]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/codegen-no-output.nudge.md",
    },
    HostMarker {
        tag: "[sandbox_error]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/codegen-sandbox-error.nudge.md",
    },
    // --- 预算 / 轮次 ---
    HostMarker {
        tag: "<loop_budget",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/react_loop/assembler.rs::build_loop_budget_hint",
    },
    // --- 题型卡（L0, 2026-08-03）---
    HostMarker {
        tag: "<query_card",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/react_loop/assembler.rs::build_query_card_block",
    },
    // --- 检索汇总 / 提示观察 ---
    HostMarker {
        tag: "[share_grounded_only]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/share-grounded-only.nudge.md",
    },
    HostMarker {
        tag: "[retrieval_summary]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/retrieval-summary.tmpl.md (prompt_assets::retrieval_summary)",
    },
    HostMarker {
        tag: "[evidence_index]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/evidence-index.tmpl.md (prompt_assets::evidence_index)",
    },
    HostMarker {
        tag: "[claim_notes]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/claim-notes.tmpl.md (prompt_assets::claim_notes)",
    },
    HostMarker {
        tag: "[working_set_trimmed]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/working-set-trimmed.nudge.md (prompt_assets::working_set_trimmed)",
    },
    HostMarker {
        tag: "[history_cleared]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/history-cleared.nudge.md (prompt_assets::history_cleared)",
    },
    HostMarker {
        tag: "<retrieval_summary>",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/retrieval-summary.tmpl.md::[retrieval_summary] 角括号仿造变体",
    },
    HostMarker {
        tag: "[blocks_skipped]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/blocks-skipped.nudge.md",
    },
    HostMarker {
        tag: "[format_hint]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/format-hint-key-value.nudge.md / format-hint-no-space-pipe.nudge.md",
    },
    // --- 能力 / 集群披露 ---
    HostMarker {
        tag: "<retrieve_cluster_index>",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/react_loop/policy/disclosure_plan.rs::render_cluster_index",
    },
    HostMarker {
        tag: "<synthesis_skill_index>",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/react_loop/policy/disclosure_plan.rs::render_cluster_index",
    },
    HostMarker {
        tag: "<docscope_metadata>",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/react_loop/policy/disclosure_plan.rs::inject_cluster_runtime_context",
    },
    // --- verify 回环观察（2026-08-07 三环）---
    HostMarker {
        tag: "[verify_feedback]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/verify-fail-synthesis.tmpl.md / verify-fail-retrieve.tmpl.md",
    },
    HostMarker {
        tag: "[draft_under_revision]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/verify-draft-under-revision.tmpl.md",
    },
    // --- evidence knockout reexposure (2026-08-07) ---
    HostMarker {
        tag: "[knockout_reexposed]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/knockout-reexposed.tmpl.md",
    },
    // --- Evidence Working Set (KEEP) priority injection (2026-08-07 W1) ---
    HostMarker {
        tag: "[ews_active]",
        forbidden_in_final: true,
        emitted_at: "crates/agent-loop/src/helpers/ews.rs::format_ews_active_block",
    },
    // --- Synthesis-time EWS recency reread (2026-08-07 W2) ---
    HostMarker {
        tag: "[evidence_reread]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/evidence-reread.tmpl.md::prompt_assets::evidence_reread_block",
    },
    // --- L2 evidence-missing structural gate (2026-08-10 scene split) ---
    HostMarker {
        tag: "[evidence_missing]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/evidence-missing.nudge.md / evidence-missing-no-client.nudge.md",
    },
    // --- SELECTED protocol fact at synthesis (2026-08-10) ---
    HostMarker {
        tag: "[selected_protocol]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/selected-protocol.nudge.md",
    },
    // --- verify fail ceiling → user-facing closeout (2026-08-10 channel philosophy) ---
    HostMarker {
        tag: "[user_facing_closeout]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/user-facing-closeout.nudge.md",
    },
    // --- Lead + Workers (2026-08-11 W0) ---
    HostMarker {
        tag: "[lead_plan_context]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/lead-plan-context.tmpl.md",
    },
    HostMarker {
        tag: "[coverage_gotcha]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/coverage-gotcha.nudge.md",
    },
    HostMarker {
        tag: "[coverage_gotcha_synth]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/coverage-gotcha-synth.nudge.md",
    },
    HostMarker {
        tag: "[evidence_pack]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/evidence-pack.tmpl.md",
    },
    HostMarker {
        tag: "[task_brief]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/task-brief.tmpl.md",
    },
    HostMarker {
        tag: "[retrieval_worklog]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/retrieval-worklog.tmpl.md",
    },
    HostMarker {
        tag: "[lead_workers_handoff]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/lead-workers-handoff-synthesis.tmpl.md",
    },
    HostMarker {
        tag: "[rebrief_wave]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/rebrief-wave.tmpl.md",
    },
    HostMarker {
        tag: "[rag_worker_sac]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/rag-worker-sac.tmpl.md",
    },
    HostMarker {
        tag: "[brief_gate_rejects]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/brief-gate-rejects.tmpl.md",
    },
    HostMarker {
        tag: "[base_tools_result]",
        forbidden_in_final: true,
        emitted_at: "prompts/loop/base-tools-result.tmpl.md",
    },
];

/// 终答中出现即违规（`forbidden_in_final = true`）的标签列表——检测器
/// `answer_contract::contains_host_observation_shell` 的派生源。
pub fn forbidden_in_final_tags() -> impl Iterator<Item = &'static str> {
    HOST_OBSERVATION_MARKERS
        .iter()
        .filter(|m| m.forbidden_in_final)
        .map(|m| m.tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const PROMPTS_LOOP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../prompts/loop");

    /// `parity_fails_on_unregistered_md_tag` 会向 `prompts/loop/` 写临时 probe
    /// 文件；`every_md_tag_candidate_is_registered` 并行扫描该目录时会撞见未登记
    /// 的 probe 而误报失败。两测试共享一把锁串行化。
    static PARITY_PROBE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 扫描 `prompts/loop/**/*.md`（含子目录 disaster），提取宿主观察标签候选。
    /// Host observation 模板约定放在 `loop/`；system/workers 技能正文不参与采集
    /// （避免 prose/伪标签噪声）。`[tag]` 仅收 snake_case 标识符。
    fn collect_md_tag_candidates() -> Vec<String> {
        let mut tags = Vec::new();
        let dir = Path::new(PROMPTS_LOOP);
        collect_md_tags_from_dir(dir, &mut tags);
        tags.sort();
        tags.dedup();
        tags
    }

    fn collect_md_tags_from_dir(dir: &Path, tags: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // One level of subdirs (e.g. workers/rag, loop/disaster).
                collect_md_tags_from_dir(&path, tags);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            for token in content
                .split('[')
                .skip(1)
                .map(|s| s.split(']').next().unwrap_or(""))
            {
                let token = token.trim();
                // Host observation tags are snake_case ascii ids only
                // (filters markdown links / Chinese prose brackets).
                if token.is_empty()
                    || !token
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    || !token.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                {
                    continue;
                }
                tags.push(format!("[{token}]"));
            }
            for token in content.split('<').skip(1) {
                let name = token.split(|c| c == '>' || c == ' ').next().unwrap_or("");
                let name = name.trim_end_matches('/');
                if name.is_empty() || name.contains(' ') {
                    continue;
                }
                if name.starts_with('/') || name.starts_with('|') {
                    continue;
                }
                let cand = format!("<{name}>");
                if cand.starts_with("<code") {
                    continue;
                }
                tags.push(cand);
            }
        }
    }

    /// parity 断言 ①：所有 md 标签候选都已在备案表中登记。
    /// 登记形态为前缀（如 `<loop_budget`）时，md 教学引用的闭合形态
    /// （`<loop_budget>`）视为该前缀的实例，允许通过。
    #[test]
    fn every_md_tag_candidate_is_registered() {
        let _guard = PARITY_PROBE_LOCK.lock().unwrap();
        let registered: Vec<&str> = HOST_OBSERVATION_MARKERS.iter().map(|m| m.tag).collect();
        for cand in collect_md_tag_candidates() {
            let known = registered.iter().any(|tag| {
                *tag == cand.as_str()
                    || cand.starts_with(tag) // 前缀形态命中闭合形态
                    || tag.strip_suffix('>') // 闭合登记形态命中其前缀候选
                        .is_some_and(|stem| cand == stem || cand.starts_with(stem))
            });
            assert!(
                known,
                "未备案标签出现在 prompts/loop/** 中: {cand}（需在 host_markers.rs 登记）"
            );
        }
    }

    /// parity 断言 ②：备案表每个发射端真实存在（.rs 源文件可搜索到
    /// tag 字面量，或 .md 文件存在）。
    #[test]
    fn every_marker_emitter_exists() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        for marker in HOST_OBSERVATION_MARKERS {
            // emitted_at 形如 `<路径>.rs::<fn>` 或 `<路径>.md (附加说明)`：
            // 取第一个 `::` 之前的路径前缀；若无 `::` 则整个视为路径，
            // 再截断到首个空白（去掉括号说明）。
            let file = marker
                .emitted_at
                .split("::")
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            let abs = Path::new(manifest)
                .join("../../")
                .join(file)
                .to_string_lossy()
                .to_string();
            let Ok(content) = std::fs::read_to_string(&abs) else {
                panic!("发射端文件不存在: {file}（备案表 emitted_at 与代码不符）");
            };
            if file.ends_with(".rs") {
                // Rust 发射端必须引用该 tag 字面量（发射端迁移后由常量承载）。
                assert!(
                    content.contains(marker.tag),
                    "发射端 {file} 中未找到已备案标签 {tag:?}",
                    tag = marker.tag
                );
            }
            // .md 发射端：文件存在即通过——parity 断言①已保证 md 内出现的
            // 观察标签都在备案表中（md 内 tag 形态可能与登记形态同形不同写，
            // 如 `[retrieval_summary]` 的角括号仿造变体）。
        }
    }

    /// parity 断言 ③：`contains_host_observation_shell` 的检测集 = 备案表中
    /// `forbidden_in_final = true` 的子集（构造上由派生保证，此测试防回归）。
    #[test]
    fn detector_set_matches_registered_forbidden_markers() {
        let forbidden: Vec<&str> = forbidden_in_final_tags().collect();
        for tag in &forbidden {
            assert!(
                super::super::answer_contract::contains_host_observation_shell(tag),
                "已备案 forbidden 标签 {tag:?} 未被检测器命中"
            );
        }
        // 无 forbidden=false 条目（当前全部终答违规）。
        assert!(
            HOST_OBSERVATION_MARKERS
                .iter()
                .all(|m| m.forbidden_in_final),
            "当前备案表应全部 forbidden_in_final=true"
        );
    }

    /// parity 敏感度：临时加一个未登记标签，测试 ① 应变红。
    #[test]
    fn parity_fails_on_unregistered_md_tag() {
        let _guard = PARITY_PROBE_LOCK.lock().unwrap();
        let md = Path::new(PROMPTS_LOOP).join("_parity_probe.tmp.md");
        let _ = std::fs::write(&md, "[unregistered_probe_tag]\n");
        let detected = collect_md_tag_candidates()
            .iter()
            .any(|t| t == "[unregistered_probe_tag]");
        let _ = std::fs::remove_file(&md);
        assert!(
            detected,
            "parity 扫描应能探测到临时未登记标签（[unregistered_probe_tag]）"
        );
    }
}
