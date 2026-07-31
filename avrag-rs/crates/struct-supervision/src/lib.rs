//! struct-query 表格监督 loop（Rust 版，P2 supervision 工具化）。
//!
//! 定位：`scripts/struct_query_poc/supervise.py`（PoC）的 Rust 重写——消费
//! 确定性管线产出的健康报告（`pipeline.py --emit-grids` 的 JSON 中间表示），
//! 跑 6 工具薄 LLM loop，产出语义标注/修复/终态，落 per-doc DuckDB + evidence
//! sidecar（与 `pipeline.py::write_duckdb` 产物对齐）。
//!
//! 安全边界（与 Python 版一致）：LLM 永不提供单元格值；指令过 schema +
//! 确定性守卫 + SQL 复验三重夹；confidence=high 仅当全部校验通过；
//! quarantine 的表不写入 DuckDB。prompts 全在
//! `prompts/pipeline/table-supervision/`（第三人称观察式），本 crate 不含
//! LLM 指令正文。

pub mod checks;
pub mod directives;
pub mod grid;
pub mod session;
pub mod store;

pub use checks::{Check, TableReport};
pub use grid::{Grid, Row, header_sig, quote_ident, render_table_md, sanitize_headers};
pub use session::{FinalState, Session};
pub use store::EvidenceChunk;

/// 输入中间表示（`pipeline.py --emit-grids` 产出的 JSON 形状）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuperviseInput {
    #[serde(default)]
    pub doc_id: Option<String>,
    pub source_text: String,
    pub grids: Vec<Grid>,
}

impl SuperviseInput {
    pub fn from_json_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
