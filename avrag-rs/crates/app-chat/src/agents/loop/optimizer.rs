use std::collections::HashMap;

/// Cross-iteration state. Held by ReActLoop, updated after each tool execution.
/// Only tracks "token-saving" signals — no chunk content, no scores.
#[derive(Default)]
pub struct IterationProgress {
    /// chunk_id -> first iteration index where it appeared (0-based)
    chunk_first_seen: HashMap<String, u8>,
    /// current iteration index
    current_iteration: u8,
}

/// Advisor output. The sole output interface of LoopOptimizer.
#[derive(Debug)]
pub enum ContextAdjustment {
    /// No intervention needed
    None,
    /// Duplicate chunks detected: only chunk_ids and first-seen iteration, no content.
    DuplicateChunksHint {
        chunk_ids: Vec<String>,
        first_seen_at: Vec<u8>,
    },
    /// Budget warning: inform LLM this is the last N iterations, forcing careful thinking.
    BudgetWarning { remaining: u8, max: u8 },
}

impl IterationProgress {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record this iteration's chunk_ids. Only records FIRST occurrence.
    pub fn record_iteration(&mut self, iteration: u8, chunk_ids: &[String]) {
        self.current_iteration = iteration;
        for id in chunk_ids {
            self.chunk_first_seen.entry(id.clone()).or_insert(iteration);
        }
    }
}

#[derive(Default)]
pub struct LoopOptimizer;

impl LoopOptimizer {
    pub fn new() -> Self {
        Self
    }

    /// Generate context adjustment advice based on current iteration state.
    pub fn advise(
        &self,
        progress: &IterationProgress,
        current_chunk_ids: &[String],
        remaining_iterations: u8,
        max_iterations: u8,
    ) -> ContextAdjustment {
        // Rule 1 (high priority): duplicate chunk detection
        let mut dup_ids = Vec::new();
        let mut dup_first_seen = Vec::new();
        for id in current_chunk_ids {
            if let Some(&first_iter) = progress.chunk_first_seen.get(id)
                && first_iter < progress.current_iteration
            {
                dup_ids.push(id.clone());
                dup_first_seen.push(first_iter);
            }
        }
        if !dup_ids.is_empty() {
            return ContextAdjustment::DuplicateChunksHint {
                chunk_ids: dup_ids,
                first_seen_at: dup_first_seen,
            };
        }

        // Rule 2 (medium priority): budget warning when remaining == 1
        if remaining_iterations == 1 {
            return ContextAdjustment::BudgetWarning {
                remaining: 1,
                max: max_iterations,
            };
        }

        ContextAdjustment::None
    }
}

pub(crate) fn build_duplicate_hint(chunk_ids: &[String], first_seen_at: &[u8]) -> String {
    let pairs: Vec<String> = chunk_ids
        .iter()
        .zip(first_seen_at.iter())
        .map(|(id, iter)| format!("{} (round {})", id, iter + 1))
        .collect();
    format!(
        "[System hint] Some chunks returned in this round already appeared in previous rounds: {}. \
         If you believe these chunks are sufficient to answer, you may proceed to synthesis; \
         if you need additional evidence, consider using different queries or tools.",
        pairs.join(", ")
    )
}

pub(crate) fn build_budget_warning(remaining: u8, max: u8) -> String {
    format!(
        "[System hint] This is the final iteration ({} of {}). \
         Please evaluate whether current evidence is sufficient: \
         if yes, prioritize a complete answer; \
         if not, choose the highest-confidence retrieval strategy this round.",
        max - remaining + 1,
        max
    )
}

/// Extract chunk_ids from tool results.
/// Handles dense_retrieval, lexical_retrieval, graph_retrieval formats.
pub fn extract_chunk_ids(results: &[contracts::ToolResult]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for result in results {
        if let Some(data) = &result.data
            && let Some(chunks) = data.get("chunks").and_then(|v| v.as_array())
        {
            for chunk in chunks {
                if let Some(id) = chunk.get("chunk_id").and_then(|v| v.as_str())
                    && seen.insert(id.to_string())
                {
                    ids.push(id.to_string());
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_iteration_no_duplicates_returns_none() {
        let mut progress = IterationProgress::new();
        progress.record_iteration(0, &["c1".to_string(), "c2".to_string()]);
        let optimizer = LoopOptimizer::new();
        let adjustment = optimizer.advise(&progress, &["c1".to_string(), "c2".to_string()], 3, 4);
        assert!(matches!(adjustment, ContextAdjustment::None));
    }

    #[test]
    fn cross_round_duplicate_triggers_hint() {
        let mut progress = IterationProgress::new();
        progress.record_iteration(0, &["c1".to_string(), "c2".to_string()]);
        progress.record_iteration(1, &["c2".to_string(), "c3".to_string()]);
        let optimizer = LoopOptimizer::new();
        let adjustment = optimizer.advise(&progress, &["c2".to_string(), "c3".to_string()], 2, 4);
        match adjustment {
            ContextAdjustment::DuplicateChunksHint { chunk_ids, first_seen_at } => {
                assert_eq!(chunk_ids, vec!["c2"]);
                assert_eq!(first_seen_at, vec![0]);
            }
            other => panic!("expected DuplicateChunksHint, got {:?}", other),
        }
    }

    #[test]
    fn multi_round_accumulated_duplicate_triggers_hint() {
        let mut progress = IterationProgress::new();
        progress.record_iteration(0, &["c1".to_string()]);
        progress.record_iteration(1, &["c2".to_string()]);
        progress.record_iteration(2, &["c1".to_string(), "c2".to_string(), "c3".to_string()]);
        let optimizer = LoopOptimizer::new();
        let adjustment =
            optimizer.advise(&progress, &["c1".to_string(), "c2".to_string(), "c3".to_string()], 1, 4);
        match adjustment {
            ContextAdjustment::DuplicateChunksHint { chunk_ids, first_seen_at } => {
                assert_eq!(chunk_ids, vec!["c1", "c2"]);
                assert_eq!(first_seen_at, vec![0, 1]);
            }
            other => panic!("expected DuplicateChunksHint, got {:?}", other),
        }
    }

    #[test]
    fn same_round_duplicate_does_not_trigger() {
        let mut progress = IterationProgress::new();
        progress.record_iteration(0, &["c1".to_string(), "c1".to_string()]);
        let optimizer = LoopOptimizer::new();
        let adjustment = optimizer.advise(&progress, &["c1".to_string()], 3, 4);
        assert!(matches!(adjustment, ContextAdjustment::None));
    }

    #[test]
    fn budget_warning_when_remaining_is_one() {
        let progress = IterationProgress::new();
        let optimizer = LoopOptimizer::new();
        let adjustment = optimizer.advise(&progress, &[], 1, 4);
        match adjustment {
            ContextAdjustment::BudgetWarning { remaining, max } => {
                assert_eq!(remaining, 1);
                assert_eq!(max, 4);
            }
            other => panic!("expected BudgetWarning, got {:?}", other),
        }
    }

    #[test]
    fn duplicate_takes_priority_over_budget_warning() {
        let mut progress = IterationProgress::new();
        progress.record_iteration(0, &["c1".to_string()]);
        progress.record_iteration(1, &["c1".to_string()]);
        let optimizer = LoopOptimizer::new();
        let adjustment = optimizer.advise(&progress, &["c1".to_string()], 1, 4);
        assert!(
            matches!(adjustment, ContextAdjustment::DuplicateChunksHint { .. }),
            "expected DuplicateChunksHint to take priority over BudgetWarning"
        );
    }

    #[test]
    fn budget_not_warning_when_remaining_is_two() {
        let progress = IterationProgress::new();
        let optimizer = LoopOptimizer::new();
        let adjustment = optimizer.advise(&progress, &[], 2, 4);
        assert!(matches!(adjustment, ContextAdjustment::None));
    }

    #[test]
    fn extract_chunk_ids_with_dense_retrieval_data() {
        let results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({
                "chunks": [
                    {"chunk_id": "c1", "text": "hello"},
                    {"chunk_id": "c2", "text": "world"},
                ]
            })),
            trace: None,
        }];
        let ids = extract_chunk_ids(&results);
        assert_eq!(ids, vec!["c1", "c2"]);
    }

    #[test]
    fn extract_chunk_ids_with_empty_chunks() {
        let results = vec![contracts::ToolResult {
            tool: "dense_retrieval".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"chunks": []})),
            trace: None,
        }];
        let ids = extract_chunk_ids(&results);
        assert!(ids.is_empty());
    }

    #[test]
    fn extract_chunk_ids_with_non_rag_tool() {
        let results = vec![contracts::ToolResult {
            tool: "calculator".to_string(),
            version: "1.0".to_string(),
            status: contracts::ToolStatus::Ok,
            data: Some(serde_json::json!({"result": 42})),
            trace: None,
        }];
        let ids = extract_chunk_ids(&results);
        assert!(ids.is_empty());
    }
}
