use super::*;

#[test]
fn test_build_summary_user_prompt_uses_batch_fields() {
    let batch = SummaryBatch {
        batch_index: 1,
        batch_count: 2,
        token_count: 3,
        content: "ctx".to_string(),
    };
    let rendered = build_summary_user_prompt("Atlas Plan", "atlas.txt", &batch);
    assert!(rendered.contains("Batch: 1 / 2"));
    assert!(rendered.contains("Token count: 3"));
    assert!(rendered.contains("Original text:\nctx"));
}

#[test]
fn test_build_summary_batches_splits_when_budget_exceeded() {
    let long = (0..5000)
        .map(|i| format!("token{}", i))
        .collect::<Vec<_>>()
        .join(" ");
    let batches = build_summary_batches_for_limit("notes.txt", &long, 500).unwrap();
    assert!(batches.len() >= 2);
    assert!(batches.iter().all(|batch| batch.token_count <= 500));
}

#[test]
fn test_summary_batches_support_code_split_mode() {
    let code = "fn add(a: i32, b: i32) -> i32 { a + b }\nfn sub(a: i32, b: i32) -> i32 { a - b }";
    let batches = build_summary_batches_for_limit("lib.rs", code, 50).unwrap();
    assert!(!batches.is_empty());
}

#[test]
fn test_finalize_user_prompt_labels_partials() {
    let prompt = build_finalize_user_prompt(
        "Atlas Plan",
        "atlas.txt",
        &["first".to_string(), "second".to_string()],
    );
    assert!(prompt.contains("[partial_summary_1]"));
    assert!(prompt.contains("Partial summaries count: 2"));
}

#[test]
fn test_parse_summary_text_supports_block_contract() {
    let raw_output = r#"
```summary_text
【压缩目标】
- 提炼 Rust 计划
```

```json
{"language":"zh"}
```
"#;

    let summary_text = parse_summary_text(raw_output);
    assert_eq!(summary_text, "【压缩目标】\n- 提炼 Rust 计划");
}

#[test]
fn test_parse_summary_text_supports_json_envelope() {
    let raw_output = r#"
```json
{
  "summary_text": "总命题：\n- Atlas 计划",
  "summary_metadata": {
    "language": "en"
  }
}
```
"#;

    let summary_text = parse_summary_text(raw_output);
    assert_eq!(summary_text, "总命题：\n- Atlas 计划");
}

#[test]
fn test_parse_summary_text_supports_legacy_text_before_json_fence() {
    let raw_output = r#"
【宏观命题树】
1. 第一层

```json
{"language":"zh"}
```
"#;

    let summary_text = parse_summary_text(raw_output);
    assert_eq!(summary_text, "【宏观命题树】\n1. 第一层");
}

#[test]
fn test_parse_summary_text_returns_trimmed_plain_text() {
    let summary_text = parse_summary_text("  plain summary body  ");
    assert_eq!(summary_text, "plain summary body");
}
