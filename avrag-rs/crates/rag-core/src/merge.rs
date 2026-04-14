use crate::retrieval::ScoredChunk;

/// Reciprocal Rank Fusion for merging dense + sparse results
///
/// The RRF formula is: score = sum(1 / (k + rank)), where k is typically 60
pub fn rrf_merge(
    dense: Vec<ScoredChunk>,
    sparse: Vec<ScoredChunk>,
    rrf_k: usize,
) -> Vec<ScoredChunk> {
    let mut seen = std::collections::HashMap::new();

    // Process dense results (rank 0-based)
    for (rank, chunk) in dense.into_iter().enumerate() {
        let id = chunk.chunk_id;
        let score = 1.0 / (rrf_k as f32 + rank as f32);
        seen.insert(id, (score, chunk));
    }

    // Process sparse results and merge scores
    for (rank, chunk) in sparse.into_iter().enumerate() {
        let id = chunk.chunk_id;
        let score = 1.0 / (rrf_k as f32 + rank as f32);
        if let Some((existing_score, existing_chunk)) = seen.get_mut(&id) {
            *existing_score += score;
            // Keep the chunk with the higher combined score or prefer dense source
            if chunk.score > existing_chunk.score && existing_chunk.source != "dense" {
                *existing_chunk = chunk;
            }
        } else {
            seen.insert(id, (score, chunk));
        }
    }

    // Collect and sort by combined RRF score
    let mut results: Vec<_> = seen
        .into_iter()
        .map(|(_id, (rrf_score, mut chunk))| {
            chunk.score = rrf_score;
            chunk
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// Perform global RRF merge for multiple lists of candidates.
pub fn global_rrf_merge(
    lists: Vec<(Vec<ScoredChunk>, f32)>, // (chunks, weight)
    rrf_k: usize,
) -> Vec<ScoredChunk> {
    let mut seen: std::collections::HashMap<uuid::Uuid, (f32, ScoredChunk)> =
        std::collections::HashMap::new();

    for (list, weight) in lists {
        for (rank, chunk) in list.into_iter().enumerate() {
            let id = chunk.chunk_id;
            let score = (1.0 / (rrf_k as f32 + rank as f32)) * weight;
            if let Some((existing_score, existing_chunk)) = seen.get_mut(&id) {
                *existing_score += score;
                if chunk.score > existing_chunk.score {
                    *existing_chunk = chunk;
                }
            } else {
                seen.insert(id, (score, chunk));
            }
        }
    }

    let mut results: Vec<_> = seen
        .into_iter()
        .map(|(_id, (rrf_score, mut chunk))| {
            chunk.score = rrf_score;
            chunk
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// Cut to top K results
pub fn cut_top_k(chunks: Vec<ScoredChunk>, k: usize) -> Vec<ScoredChunk> {
    if k == 0 {
        return Vec::new();
    }

    let mut sorted = chunks;
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.into_iter().take(k).collect()
}

/// Apply dual threshold cut: keep all above threshold, but ensure at least min_k (if available).
pub fn dual_threshold_cut(
    chunks: Vec<ScoredChunk>,
    min_k: usize,
    score_threshold: f32,
) -> Vec<ScoredChunk> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    for (i, chunk) in chunks.into_iter().enumerate() {
        if chunk.score >= score_threshold || i < min_k {
            results.push(chunk);
        } else {
            break;
        }
    }
    results
}
