use crate::retrieval::ScoredChunk;
use rayon::prelude::*;

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

    results.par_sort_by(|a, b| {
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

    results.par_sort_by(|a, b| {
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

/// Env: `RETRIEVAL_ADJACENT_MERGE` — product default **on** (chunk semantic
/// continuity: S-anchor + L-neighborhood). Set `0`/`false`/`off` to disable.
pub fn adjacent_merge_enabled() -> bool {
    match std::env::var("RETRIEVAL_ADJACENT_MERGE") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off"))
        }
        // Default on: list/semantic break across cursor neighbors (q017 class).
        Err(_) => true,
    }
}

/// S-anchor + L-neighborhood merge by same-doc `cursor` (design
/// `2026-08-05-retrieval-adjacent-shortlist-merge-design.md`).
///
/// - Neighbors in S: join content in cursor order into one evidence row.
/// - Neighbors only in L: pull into the package (capped by `pull_budget`).
/// - Missing cursor on either side → skip that pair (safe degrade).
/// - Table chunks do not merge with text.
pub fn adjacent_merge_shortlist_longlist(
    shortlist: Vec<ScoredChunk>,
    longlist: &[ScoredChunk],
    radius: i32,
    pull_budget: usize,
) -> Vec<ScoredChunk> {
    if shortlist.is_empty() || radius < 0 {
        return shortlist;
    }
    let radius = radius.max(0);
    let s_ids: std::collections::HashSet<uuid::Uuid> =
        shortlist.iter().map(|c| c.chunk_id).collect();
    let l_by_key: std::collections::HashMap<(uuid::Uuid, i32), &ScoredChunk> = longlist
        .iter()
        .filter_map(|c| c.cursor.map(|cur| ((c.doc_id, cur), c)))
        .collect();

    let mut consumed: std::collections::HashSet<uuid::Uuid> = std::collections::HashSet::new();
    let mut out: Vec<ScoredChunk> = Vec::new();
    let mut pulled = 0usize;

    for anchor in shortlist {
        if !consumed.insert(anchor.chunk_id) {
            continue;
        }
        let Some(a_cur) = anchor.cursor else {
            out.push(anchor);
            continue;
        };
        if is_table_chunk(&anchor) {
            out.push(anchor);
            continue;
        }

        let mut run: Vec<ScoredChunk> = vec![anchor.clone()];
        for delta in -radius..=radius {
            if delta == 0 {
                continue;
            }
            let key = (anchor.doc_id, a_cur + delta);
            let Some(neigh) = l_by_key.get(&key).copied() else {
                continue;
            };
            if is_table_chunk(neigh) {
                continue;
            }
            if s_ids.contains(&neigh.chunk_id) {
                if consumed.insert(neigh.chunk_id) {
                    run.push(neigh.clone());
                }
            } else if pulled < pull_budget && consumed.insert(neigh.chunk_id) {
                run.push(neigh.clone());
                pulled += 1;
            }
        }

        run.sort_by_key(|c| c.cursor.unwrap_or(i32::MAX));
        let score = run
            .iter()
            .filter(|c| s_ids.contains(&c.chunk_id))
            .map(|c| c.score)
            .fold(anchor.score, f32::max);
        // Anchor = highest-score S member for cite; keep its id as primary chunk_id.
        let anchor_best = run
            .iter()
            .filter(|c| s_ids.contains(&c.chunk_id))
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(&run[0]);
        let mut merged = anchor_best.clone();
        merged.score = score;
        merged.member_chunk_ids = run.iter().map(|c| c.chunk_id).collect();
        if run.len() > 1 {
            merged.content = run
                .iter()
                .map(|c| c.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            if !merged.source.contains("+adjacent") {
                merged.source = format!("{}+adjacent", merged.source);
            }
            // Prefer min cursor row's page for display continuity.
            merged.page = run[0].page;
            merged.cursor = run[0].cursor;
        }
        out.push(merged);
    }
    out
}

fn is_table_chunk(c: &ScoredChunk) -> bool {
    let t = c.chunk_type.to_ascii_lowercase();
    t.contains("table") || t == "struct" || t == "row_group"
}

/// Hydrate `cursor` from content-store metadata when missing on scored hits.
pub async fn hydrate_cursors_from_store(
    store: &dyn crate::ports::ContentStore,
    auth: &contracts::auth_runtime::AuthContext,
    chunks: &mut [ScoredChunk],
) {
    let missing: Vec<uuid::Uuid> = chunks
        .iter()
        .filter(|c| c.cursor.is_none())
        .map(|c| c.chunk_id)
        .collect();
    if missing.is_empty() {
        return;
    }
    let Ok(map) = store.get_chunks_by_ids(auth, &missing).await else {
        return;
    };
    for c in chunks.iter_mut() {
        if c.cursor.is_some() {
            continue;
        }
        if let Some(ic) = map.get(&c.chunk_id) {
            c.cursor = avrag_retrieval_data_plane::cursor_from_value(Some(&ic.metadata));
        }
    }
}

#[cfg(test)]
mod adjacent_tests {
    use super::*;
    use uuid::Uuid;

    fn ch(id: u128, doc: u128, cur: i32, score: f32, text: &str) -> ScoredChunk {
        ScoredChunk::new_text(
            Uuid::from_u128(id),
            Uuid::from_u128(doc),
            text.to_string(),
            score,
            "dense".into(),
            None,
        )
        .with_cursor(Some(cur))
    }

    #[test]
    fn pulls_neighbor_from_longlist() {
        let s = vec![ch(1, 9, 5, 0.9, "a")];
        let l = vec![ch(1, 9, 5, 0.9, "a"), ch(2, 9, 6, 0.5, "b")];
        let out = adjacent_merge_shortlist_longlist(s, &l, 1, 8);
        assert_eq!(out.len(), 1);
        assert!(out[0].content.contains("a") && out[0].content.contains("b"));
        assert_eq!(out[0].member_chunk_ids.len(), 2);
        assert!(out[0].source.contains("+adjacent"));
    }

    #[test]
    fn merges_two_shortlist_neighbors_into_one() {
        let s = vec![ch(1, 9, 5, 0.9, "a"), ch(2, 9, 6, 0.8, "b")];
        let l = s.clone();
        let out = adjacent_merge_shortlist_longlist(s, &l, 1, 8);
        assert_eq!(out.len(), 1);
        assert!(out[0].content.contains("a") && out[0].content.contains("b"));
        assert_eq!(out[0].members().len(), 2);
    }

    #[test]
    fn skips_without_cursor() {
        let mut a = ch(1, 9, 5, 0.9, "a");
        a.cursor = None;
        let out = adjacent_merge_shortlist_longlist(vec![a], &[], 1, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].content, "a");
    }
}
