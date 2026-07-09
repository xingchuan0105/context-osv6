//! Judge calibration runner.
//!
//! Usage:
//!   cargo run -p rag_quality --bin judge_calibration -- tests/rag_quality/golden_set_calibration.json
//!   cargo run -p rag_quality --bin judge_calibration -- tests/rag_quality/golden_set_calibration.json --llm
//!
//! The calibration JSON may contain `manual_faithfulness` per example as either
//! bool or numeric score. Missing labels are reported as pending.

use rag_quality::{
    CitedChunk, CitedChunks, FaithfulnessInput, FaithfulnessJudge, LlmNliJudge,
    SubstringFaithfulnessJudge, cohen_kappa_binary,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        anyhow::bail!("usage: judge_calibration <calibration.json> [--llm]");
    }
    let use_llm = args.iter().any(|a| a == "--llm");
    args.retain(|a| a != "--llm");
    let path = std::path::PathBuf::from(&args[0]);
    let raw = std::fs::read_to_string(&path)?;
    let dataset: serde_json::Value = serde_json::from_str(&raw)?;

    let judge: Box<dyn FaithfulnessJudge> = if use_llm {
        Box::new(LlmNliJudge::from_agent_env()?)
    } else {
        Box::new(SubstringFaithfulnessJudge)
    };

    let mut total = 0usize;
    let mut labeled = 0usize;
    let mut manual = Vec::new();
    let mut predicted = Vec::new();

    for example in dataset
        .get("subsets")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .flat_map(|subset| {
            subset
                .get("examples")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
        })
    {
        total += 1;
        let query = example
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let answer = example
            .get("expected_answer")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cited_chunks = CitedChunks {
            chunks: source_chunks_as_citations(example),
        };
        let input = FaithfulnessInput {
            query,
            answer,
            cited_chunks,
        };
        let judgment = judge.judge(&input).await?;
        let pred = judgment.faithfulness >= 0.85;
        if let Some(label) = manual_label(example) {
            labeled += 1;
            manual.push(label);
            predicted.push(pred);
        }
    }

    println!("Judge calibration: {}", path.display());
    println!("  judge: {}", if use_llm { "llm_nli" } else { "substring" });
    println!("  total examples: {total}");
    println!("  labeled examples: {labeled}");
    if labeled == 0 {
        println!("  kappa: pending (manual_faithfulness labels are empty)");
        println!("  gate: pending; do not hard-gate LLM judge until kappa >= 0.60");
    } else if let Some(kappa) = cohen_kappa_binary(&manual, &predicted) {
        println!("  kappa: {kappa:.3}");
        println!(
            "  gate: {}",
            if kappa >= 0.60 {
                "eligible (kappa >= 0.60)"
            } else {
                "not eligible (kappa < 0.60)"
            }
        );
    } else {
        println!("  kappa: undefined (label distribution has zero variance)");
    }

    Ok(())
}

fn source_chunks_as_citations(example: &serde_json::Value) -> Vec<CitedChunk> {
    example
        .get("source_chunks")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(idx, chunk)| {
            let content = match chunk.get("type").and_then(|v| v.as_str()) {
                Some("substring") => chunk.get("text").and_then(|v| v.as_str())?,
                Some("keywords") => {
                    return Some(CitedChunk {
                        chunk_id: Some(format!("golden-{idx}")),
                        citation_id: idx as i64 + 1,
                        content: chunk
                            .get("keywords")
                            .and_then(|v| v.as_array())
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                        score: 1.0,
                    });
                }
                _ => return None,
            };
            Some(CitedChunk {
                chunk_id: Some(format!("golden-{idx}")),
                citation_id: idx as i64 + 1,
                content: content.to_string(),
                score: 1.0,
            })
        })
        .collect()
}

fn manual_label(example: &serde_json::Value) -> Option<bool> {
    let v = example.get("manual_faithfulness")?;
    if let Some(b) = v.as_bool() {
        return Some(b);
    }
    v.as_f64().map(|score| score >= 0.85)
}
