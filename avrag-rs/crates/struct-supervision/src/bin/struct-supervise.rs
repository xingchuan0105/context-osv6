//! CLI：`struct-supervise <input.grids.json> --out <doc>.duckdb [--report sup.json]
//! [--max-turns 40] [--dry-run]`（flag 与 `supervise.py` 对齐）。

use std::path::PathBuf;

use avrag_struct_supervision::{SuperviseConfig, SuperviseInput, runner::supervise};

fn parse_args() -> anyhow::Result<(PathBuf, PathBuf, Option<PathBuf>, usize, bool, Option<String>)> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        anyhow::bail!("usage: struct-supervise <input.grids.json> --out <doc>.duckdb [--report sup.json] [--max-turns 40] [--dry-run]");
    }
    let input = PathBuf::from(&args[0]);
    let mut out: Option<PathBuf> = None;
    let mut report: Option<PathBuf> = None;
    let mut max_turns: usize = 40;
    let mut dry_run = false;
    let mut doc_id: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--report" => {
                report = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--max-turns" => {
                max_turns = args[i + 1].parse()?;
                i += 2;
            }
            "--doc-id" => {
                doc_id = Some(args[i + 1].clone());
                i += 2;
            }
            "--dry-run" => {
                dry_run = true;
                i += 1;
            }
            other => anyhow::bail!("未知参数:{other}"),
        }
    }
    Ok((input, out.unwrap_or_else(|| PathBuf::from("/tmp/sup_doc.duckdb")), report, max_turns, dry_run, doc_id))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (input_path, out_path, report_path, max_turns, dry_run, doc_id) = parse_args()?;
    let bytes = std::fs::read(&input_path)?;
    let mut input = SuperviseInput::from_json_bytes(&bytes)?;
    if doc_id.is_some() {
        input.doc_id = doc_id;
    }

    let cfg = SuperviseConfig {
        max_turns,
        doc_name: input_path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "doc".into()),
        out_path,
        report_path,
    };

    if dry_run {
        let session = avrag_struct_supervision::session::Session::new(&input)?;
        println!("{}", session.briefing(&cfg.doc_name));
        return Ok(());
    }

    let Some(llm) = avrag_struct_supervision::config::llm_client_from_env() else {
        anyhow::bail!("INGESTION_LLM_BASE_URL / INGESTION_LLM_API_KEY 未配置（见 avrag-rs/.env）");
    };
    let rep = supervise(&input, &llm, &cfg).await?;
    let mut out = serde_json::to_value(&rep)?;
    if let Some(obj) = out.as_object_mut() {
        obj.remove("log");
    }
    let pretty = serde_json::to_string_pretty(&out)?;
    println!("{}", pretty.chars().take(3000).collect::<String>());
    Ok(())
}
