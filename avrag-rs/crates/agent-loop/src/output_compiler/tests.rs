//! Per-code unit tests for the worker handoff compiler (S1).

use super::handoff::{HandoffCompileInput, compile_handoff, observed_chunk_ids};
use super::types::Severity;
use std::collections::HashSet;

fn observed(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn compile(raw: &str, obs: Option<&HashSet<String>>, has_tools: bool) -> super::CompileOutcome<serde_json::Value> {
    compile_handoff(&HandoffCompileInput {
        raw,
        observed_chunk_ids: obs,
        has_tool_results: has_tools,
    })
}

fn codes(outcome: &super::CompileOutcome<serde_json::Value>) -> Vec<String> {
    outcome.diagnostic_codes()
}

fn valid_handoff_json() -> &'static str {
    r#"{"schema_version":"internal_worker_handoff_v1","summary":"2019年建厂","key_facts":[{"claim":"2019年建厂","evidence":["c1"]}],"coverage":"full","gaps":[]}"#
}

// ---- E101 -----------------------------------------------------------------

#[test]
fn task_result_wrapper_is_e101() {
    // q045: correct conclusion in a self-invented envelope.
    let raw = r#"{"task_result":{"summary":"文中未写明总部城市","coverage":"insufficient"}}"#;
    let outcome = compile(raw, None, false);
    assert!(outcome.value.is_none());
    assert!(outcome.has_errors());
    assert_eq!(codes(&outcome), vec!["E101"]);
    let fb = outcome.render_feedback();
    assert!(fb.contains("E101"), "{fb}");
    assert!(fb.contains("internal_worker_handoff_v1"), "{fb}");
    assert!(fb.contains("请按契约重新输出"), "{fb}");
}

#[test]
fn prose_and_raw_code_block_are_e101() {
    for raw in [
        "散文式摘要：文档讲了三件事",
        "<code language=\"python\">\nchunks = await client.dense_search(query=\"保修\")\n</code>",
        "",
    ] {
        let outcome = compile(raw, None, false);
        assert!(outcome.value.is_none(), "{raw}");
        assert!(codes(&outcome).contains(&"E101".to_string()), "{raw}");
    }
}

// ---- E102 -----------------------------------------------------------------

#[test]
fn missing_key_facts_with_tool_results_is_e102() {
    // q087: {"handoff": true, "summary": …} with no key_facts while the loop
    // observed chunks.
    let obs = observed(&["c1", "c2"]);
    let raw = r#"{"handoff": true, "summary":"只找到一条","coverage":"partial","gaps":[]}"#;
    let outcome = compile(raw, Some(&obs), true);
    assert!(outcome.has_errors());
    assert!(codes(&outcome).contains(&"E102".to_string()));
    // Value survives so post-loop can still build a degraded handoff.
    assert!(outcome.value.is_some());
    let fb = outcome.render_feedback();
    assert!(fb.contains("E102"), "{fb}");
    assert!(fb.contains("c1"), "legal pointers listed: {fb}");
}

#[test]
fn insufficient_coverage_carveout_is_not_e102() {
    // 查无即成功 (design §3.1): insufficient + no facts + gaps is a full delivery.
    let obs = observed(&["c1"]);
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"文档未记载保修年限","key_facts":[],"coverage":"insufficient","gaps":["保修年限"]}"#;
    let outcome = compile(raw, Some(&obs), true);
    assert!(
        !outcome.has_errors(),
        "diagnostics: {:?}",
        outcome.diagnostics
    );
}

#[test]
fn no_tool_results_never_e102() {
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","coverage":"partial","gaps":[]}"#;
    let outcome = compile(raw, None, false);
    assert!(!outcome.has_errors());
}

// ---- E103 -----------------------------------------------------------------

#[test]
fn unobserved_pointer_is_e103_and_fact_dropped() {
    let obs = observed(&["c1"]);
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"real","evidence":["c1"]},{"claim":"fake","evidence":["c999"]}],"coverage":"full","gaps":[]}"#;
    let outcome = compile(raw, Some(&obs), true);
    assert!(outcome.has_errors());
    assert!(codes(&outcome).contains(&"E103".to_string()));
    let v = outcome.value.expect("value survives");
    let facts = v["key_facts"].as_array().unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["claim"], "real");
    assert_eq!(v["coverage"], "full", "some facts survived → coverage stays");
}

#[test]
fn all_facts_dropped_downgrades_coverage() {
    let obs = observed(&["c1"]);
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"fake","evidence":["c999"]}],"coverage":"full","gaps":[]}"#;
    let outcome = compile(raw, Some(&obs), true);
    assert!(!codes(&outcome).contains(&"E102".to_string()));
    let v = outcome.value.expect("value survives");
    assert_eq!(v["key_facts"].as_array().unwrap().len(), 0);
    assert_eq!(v["coverage"], "insufficient");
}

#[test]
fn facts_without_pointers_survive_e103() {
    // C4 semantics: a claim with NO evidence pointers is not dropped.
    let obs = observed(&["c1"]);
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"no pointers","evidence":[]}],"coverage":"partial","gaps":[]}"#;
    let outcome = compile(raw, Some(&obs), true);
    assert!(!outcome.has_errors(), "{:?}", outcome.diagnostics);
    assert_eq!(outcome.value.unwrap()["key_facts"].as_array().unwrap().len(), 1);
}

#[test]
fn unknown_run_context_skips_e103() {
    // Pure parse (no tool trail): pointer truthfulness cannot be checked.
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"x","evidence":["c999"]}],"coverage":"full","gaps":[]}"#;
    let outcome = compile(raw, None, false);
    assert!(!outcome.has_errors());
    assert_eq!(outcome.value.unwrap()["key_facts"].as_array().unwrap().len(), 1);
}

// ---- E104 -----------------------------------------------------------------

#[test]
fn fabricated_execution_result_is_stripped_with_warning() {
    // q039: fabricated execution output inside an otherwise valid handoff.
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"见 <code_execution_result>韩方投资者 B株式会社 持股40%</code_execution_result> 所示","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
    let outcome = compile(raw, None, false);
    assert!(!outcome.has_errors(), "E104 is a warning: {:?}", outcome.diagnostics);
    assert!(codes(&outcome).contains(&"E104".to_string()));
    let v = outcome.value.expect("value survives");
    let summary = v["summary"].as_str().unwrap();
    assert!(!summary.contains("B株式会社"), "{summary}");
    assert!(!summary.contains("code_execution_result"), "{summary}");
    assert_eq!(
        outcome.diagnostics.iter().find(|d| d.code == "E104").unwrap().severity,
        Severity::Warning
    );
}

#[test]
fn fabricated_block_inside_claim_is_stripped() {
    let obs = observed(&["c1"]);
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"持股 <code_execution_result untrusted=\"true\">40%</code_execution_result> 见证据","evidence":["c1"]}],"coverage":"full","gaps":[]}"#;
    let outcome = compile(raw, Some(&obs), true);
    assert!(!outcome.has_errors(), "{:?}", outcome.diagnostics);
    let v = outcome.value.unwrap();
    let claim = v["key_facts"][0]["claim"].as_str().unwrap();
    assert!(!claim.contains("40%"), "{claim}");
    assert!(claim.contains("见证据"), "{claim}");
}

// ---- Warnings -------------------------------------------------------------

#[test]
fn fenced_json_is_tolerated_with_w102() {
    let raw = "```json\n{\"summary\":\"s\",\"coverage\":\"full\",\"gaps\":[],\"key_facts\":[]}\n```";
    let outcome = compile(raw, None, false);
    assert!(!outcome.has_errors());
    assert!(codes(&outcome).contains(&"W102".to_string()));
    assert_eq!(outcome.value.unwrap()["summary"], "s");
}

#[test]
fn hedge_markers_raise_w101_advisory() {
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"访谈可能覆盖了全部营销人员","key_facts":[{"claim":"从上下文推断应覆盖","evidence":[]}],"coverage":"partial","gaps":[]}"#;
    let outcome = compile(raw, None, false);
    assert!(!outcome.has_errors(), "W101 is advisory only");
    assert!(codes(&outcome).contains(&"W101".to_string()));
}

#[test]
fn clean_handoff_compiles_without_diagnostics() {
    let obs = observed(&["c1"]);
    let outcome = compile(valid_handoff_json(), Some(&obs), true);
    assert!(!outcome.has_errors());
    assert!(
        outcome.diagnostics.is_empty(),
        "clean handoff → no diagnostics: {:?}",
        outcome.diagnostics
    );
    assert_eq!(outcome.value.unwrap()["summary"], "2019年建厂");
}

#[test]
fn legacy_internal_answer_v1_accepted() {
    let raw = r#"{"schema_version":"internal_answer_v1","answer_text":"结论正文","coverage":"full"}"#;
    let outcome = compile(raw, None, false);
    assert!(!outcome.has_errors(), "{:?}", outcome.diagnostics);
    assert!(outcome.value.is_some());
}

// ---- observed id harvesting -----------------------------------------------

#[test]
fn observed_chunk_ids_harvest_both_shapes_and_skip_non_ok() {
    let mk = |status: contracts::ToolStatus, data: serde_json::Value| contracts::ToolResult {
        tool: "dense_retrieval".into(),
        version: "1".into(),
        status,
        data: Some(data),
        trace: None,
    };
    let results = vec![
        mk(
            contracts::ToolStatus::Ok,
            serde_json::json!([{"chunk_id": "a"}, {"chunk_id": "b"}]),
        ),
        mk(
            contracts::ToolStatus::Ok,
            serde_json::json!({"chunks": [{"chunk_id": "c"}]}),
        ),
        mk(
            contracts::ToolStatus::Error,
            serde_json::json!([{"chunk_id": "nope"}]),
        ),
    ];
    let ids = observed_chunk_ids(&results);
    assert!(ids.contains("a") && ids.contains("b") && ids.contains("c"));
    assert!(!ids.contains("nope"), "non-Ok results never count");
}
