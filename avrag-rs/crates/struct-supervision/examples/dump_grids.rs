// parity 工具：Rust prepare() → 与 `pipeline.py --emit-grids` 同形状 JSON（diff 用）。
fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: dump_grids <input.md>"))?;
    let text = std::fs::read_to_string(&path)?;
    let input = avrag_struct_supervision::SuperviseInput::from_markdown(None, text);
    // 与 pipeline --emit-grids 的 grids 段同形状（doc_id/source_text 不参与 diff）。
    let grids: Vec<serde_json::Value> = input
        .grids
        .iter()
        .map(|g| {
            serde_json::json!({
                "start_line": g.start_line,
                "notes": g.notes,
                "rows": g.rows.iter().map(|r| serde_json::json!({"line": r.line, "cells": r.cells})).collect::<Vec<_>>(),
            })
        })
        .collect();
    println!("{}", serde_json::to_string(&serde_json::json!({"grids": grids}))?);
    Ok(())
}
