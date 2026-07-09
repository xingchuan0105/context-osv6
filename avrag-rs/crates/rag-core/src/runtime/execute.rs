//! Channel budget helpers retained after multi-channel ExecutePlan harness removal.
//!
//! Per-tool retrieval lives in `runtime::tools`. These pure functions remain for
//! budgeting tests and any future multi-channel orchestration that is *not*
//! productized as ExecutePlanRequest.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChannelCandidateBudgets {
    pub(super) text_dense: usize,
    pub(super) bm25: usize,
    pub(super) multimodal_dense: usize,
    pub(super) graph: usize,
}

pub(super) fn default_channel_candidate_budgets(
    total_candidate_budget: usize,
) -> ChannelCandidateBudgets {
    let weights = [35usize, 25, 15, 25];
    let total_weight = weights.iter().sum::<usize>();
    let mut budgets = weights
        .iter()
        .map(|weight| (total_candidate_budget * *weight) / total_weight)
        .collect::<Vec<_>>();
    let assigned = budgets.iter().sum::<usize>();
    let mut remainders = weights
        .iter()
        .enumerate()
        .map(|(index, weight)| {
            (
                index,
                (total_candidate_budget * *weight) % total_weight,
                *weight,
            )
        })
        .collect::<Vec<_>>();
    remainders.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    for (index, _, _) in remainders
        .into_iter()
        .take(total_candidate_budget.saturating_sub(assigned))
    {
        budgets[index] += 1;
    }

    ChannelCandidateBudgets {
        text_dense: budgets[0],
        bm25: budgets[1],
        multimodal_dense: budgets[2],
        graph: budgets[3],
    }
}

pub(super) fn graph_final_context_budget(
    final_chunk_budget: usize,
    graph_chunk_count: usize,
) -> usize {
    if final_chunk_budget == 0 || graph_chunk_count == 0 {
        return 0;
    }
    final_chunk_budget.div_ceil(5).max(1).min(graph_chunk_count)
}
