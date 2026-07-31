//! 6 工具 schema（对齐 `supervise.TOOL_SCHEMAS`）+ 分发执行。

use contracts::ToolSpec;

fn spec(
    name: &str,
    description: &str,
    properties: serde_json::Value,
    required: &[&str],
) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        version: "v1".to_string(),
        description: description.to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
        output_schema: serde_json::json!({}),
    }
}

/// 6 工具 schema（annotate/fetch_slice/run_check/apply_directive/quarantine/done）。
pub fn specs() -> Vec<ToolSpec> {
    vec![
        spec(
            "annotate",
            "批量语义标注并给出终态置信度",
            serde_json::json!({
                "tables": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "table_id": {"type": "string"},
                            "caption": {"type": "string"},
                            "unit": {"type": "string"},
                            "column_semantics": {"type": "object"},
                            "table_kind": {"type": "string", "enum": ["detail", "summary", "kv", "layout"]},
                            "confidence": {"type": "string", "enum": ["high", "low"]},
                        },
                        "required": ["table_id", "table_kind", "confidence"],
                    }
                }
            }),
            &["tables"],
        ),
        spec(
            "fetch_slice",
            "取表的有界切片",
            serde_json::json!({
                "table_id": {"type": "string"},
                "row_range": {"type": "array", "items": {"type": "integer"}},
                "source_lines": {"type": "array", "items": {"type": "integer"}},
            }),
            &["table_id"],
        ),
        spec(
            "run_check",
            "在表存储上跑只读校验 SQL",
            serde_json::json!({"sql": {"type": "string"}}),
            &["sql"],
        ),
        spec(
            "apply_directive",
            "应用修复指令并重跑复验",
            serde_json::json!({
                "table_id": {"type": "string"},
                "directive": {"type": "object"},
            }),
            &["table_id", "directive"],
        ),
        spec(
            "quarantine",
            "隔离表(不入查询侧)",
            serde_json::json!({
                "table_id": {"type": "string"},
                "reason": {"type": "string"},
            }),
            &["table_id", "reason"],
        ),
        spec(
            "done",
            "全部表有终态后结束",
            serde_json::json!({"summary": {"type": "string"}}),
            &[],
        ),
    ]
}

/// 分发一次工具调用；`done` 返回 Ok(Some(summary)) 表示会话结束。
/// 返回工具观察文本（第三人称观察式，作 tool 回合内容）。
pub fn dispatch(
    session: &mut crate::session::Session,
    call: &contracts::ToolCall,
    log: &mut Vec<(String, serde_json::Value, String)>,
) -> Result<Option<String>, String> {
    let args = call.args.clone();
    match call.tool.as_str() {
        "annotate" => {
            let tables = args
                .get("tables")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let out = session.t_annotate(&tables);
            log.push(("annotate".into(), args, out.clone()));
            Ok(None)
        }
        "fetch_slice" => {
            let out = session.t_fetch_slice(&args);
            log.push(("fetch_slice".into(), args, out.clone()));
            Ok(None)
        }
        "run_check" => {
            let out = session.t_run_check(&args);
            log.push(("run_check".into(), args, out.clone()));
            Ok(None)
        }
        "apply_directive" => {
            let out = session.t_apply_directive(&args);
            log.push(("apply_directive".into(), args, out.clone()));
            Ok(None)
        }
        "quarantine" => {
            let out = session.t_quarantine(&args);
            log.push(("quarantine".into(), args, out.clone()));
            Ok(None)
        }
        "done" => {
            let summary = args
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            log.push(("done".into(), args, "监督结束。".into()));
            Ok(Some(summary))
        }
        other => Err(format!("未知工具:{other}")),
    }
}
