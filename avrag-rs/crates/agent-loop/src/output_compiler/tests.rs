//! Per-code unit tests for the worker handoff compiler (K3 slimmed table).

use super::handoff::{HandoffCompileInput, compile_handoff};
use super::types::Severity;

fn compile(raw: &str, has_tools: bool) -> super::CompileOutcome<serde_json::Value> {
    compile_handoff(&HandoffCompileInput {
        raw,
        has_tool_results: has_tools,
    })
}

fn codes(outcome: &super::CompileOutcome<serde_json::Value>) -> Vec<String> {
    outcome.diagnostic_codes()
}

// ---- K3: prose / SELECTED-only / non-envelope JSON are all legal ----------

#[test]
fn prose_handoff_compiles_clean() {
    // K3: the handoff contract is prose + optional SELECTED line — no JSON
    // required, no error, no continuation.
    let outcome = compile("文档确认烟台冰轮是主要竞争对手，但未记载其总部城市。", false);
    assert!(outcome.value.is_none());
    assert!(!outcome.has_errors());
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
}

#[test]
fn selected_only_message_compiles_clean() {
    let outcome = compile("SELECTED: #2, #5, #9", true);
    assert!(outcome.value.is_none());
    assert!(!outcome.has_errors());
}

#[test]
fn task_result_wrapper_no_longer_rejected() {
    // K3: the old E101 envelope check is retired — a self-invented JSON
    // wrapper compiles like anything else (downstream treats it as prose).
    let raw = r#"{"task_result":{"summary":"文中未写明总部城市","coverage":"insufficient"}}"#;
    let outcome = compile(raw, true);
    assert!(!outcome.has_errors());
    assert!(outcome.value.is_some(), "JSON still parses as a value");
    assert!(!codes(&outcome).contains(&"E101".to_string()));
}

#[test]
fn raw_code_block_compiles_clean() {
    // q087 class: a stray code block used to be E101; K3 treats it as prose
    // (E104 fabrication stripping still applies elsewhere).
    let outcome = compile("<code language=\"python\">\nprint(1)\n</code>", true);
    assert!(!outcome.has_errors());
}

#[test]
fn empty_output_compiles_to_none() {
    let outcome = compile("   ", false);
    assert!(outcome.value.is_none());
    assert!(!outcome.has_errors());
}

// ---- E105 (kept) -----------------------------------------------------------

#[test]
fn insufficient_with_zero_retrieval_calls_is_e105() {
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"未找到相关信息","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
    let outcome = compile(raw, false);
    assert!(outcome.has_errors());
    assert!(codes(&outcome).contains(&"E105".to_string()));
    let fb = outcome.render_feedback();
    assert!(fb.contains("E105"), "{fb}");
    assert!(fb.contains("零检索调用"), "{fb}");
    assert!(fb.contains("先执行至少一次检索"), "{fb}");
}

#[test]
fn insufficient_with_retrieval_calls_is_not_e105() {
    // A zero-chunk Ok result still counts as having retrieved (合法查无).
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"文档未记载保修年限","key_facts":[],"coverage":"insufficient","gaps":["保修年限"]}"#;
    let outcome = compile(raw, true);
    assert!(!outcome.has_errors(), "{:?}", outcome.diagnostics);
}

#[test]
fn partial_coverage_with_zero_calls_is_not_e105() {
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[],"coverage":"partial","gaps":[]}"#;
    let outcome = compile(raw, false);
    assert!(
        !codes(&outcome).contains(&"E105".to_string()),
        "{:?}",
        outcome.diagnostics
    );
}

#[test]
fn prose_never_triggers_e105() {
    // E105 reads the declared coverage field; prose has none.
    let outcome = compile("查无相关内容。", false);
    assert!(!outcome.has_errors());
}

// ---- E104 (kept) -----------------------------------------------------------

#[test]
fn fabricated_execution_result_is_stripped_with_warning() {
    // q039: fabricated execution output inside an otherwise valid handoff.
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"见 <code_execution_result>韩方投资者 B株式会社 持股40%</code_execution_result> 所示","key_facts":[],"coverage":"insufficient","gaps":["x"]}"#;
    // has_tools=true: a retrieval happened, so the insufficient coverage is a
    // legal 查无 — this test is about E104 stripping, not E105.
    let outcome = compile(raw, true);
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
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"s","key_facts":[{"claim":"持股 <code_execution_result untrusted=\"true\">40%</code_execution_result> 见证据","evidence":["c1"]}],"coverage":"full","gaps":[]}"#;
    let outcome = compile(raw, true);
    assert!(!outcome.has_errors(), "{:?}", outcome.diagnostics);
    let v = outcome.value.unwrap();
    let claim = v["key_facts"][0]["claim"].as_str().unwrap();
    assert!(!claim.contains("40%"), "{claim}");
    assert!(claim.contains("见证据"), "{claim}");
}

// ---- Warnings ---------------------------------------------------------------

#[test]
fn fenced_json_is_tolerated_with_w102() {
    let raw = "```json\n{\"summary\":\"s\",\"coverage\":\"full\",\"gaps\":[],\"key_facts\":[]}\n```";
    let outcome = compile(raw, false);
    assert!(!outcome.has_errors());
    assert!(codes(&outcome).contains(&"W102".to_string()));
    assert_eq!(outcome.value.unwrap()["summary"], "s");
}

#[test]
fn hedge_markers_raise_w101_advisory() {
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"访谈可能覆盖了全部营销人员","key_facts":[{"claim":"从上下文推断应覆盖","evidence":[]}],"coverage":"partial","gaps":[]}"#;
    let outcome = compile(raw, true);
    assert!(!outcome.has_errors(), "W101 is advisory only");
    assert!(codes(&outcome).contains(&"W101".to_string()));
}

#[test]
fn clean_json_handoff_compiles_without_diagnostics() {
    let raw = r#"{"schema_version":"internal_worker_handoff_v1","summary":"2019年建厂","key_facts":[{"claim":"2019年建厂","evidence":["c1"]}],"coverage":"full","gaps":[]}"#;
    let outcome = compile(raw, true);
    assert!(!outcome.has_errors());
    assert!(
        outcome.diagnostics.is_empty(),
        "clean handoff → no diagnostics: {:?}",
        outcome.diagnostics
    );
    assert_eq!(outcome.value.unwrap()["summary"], "2019年建厂");
}
