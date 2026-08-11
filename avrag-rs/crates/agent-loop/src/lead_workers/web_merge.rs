//! Host multi-query web fan-out: merge multiple [`SearchResponse`] into one hit list.
//!
//! Pure functions only (W0). Concurrent I/O is W1.

use avrag_search::{SearchResponse, SearchResult};
use serde::{Deserialize, Serialize};

/// One web hit after multi-query merge (stable `web:n` alias order).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergedWebHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// 1-based index for `[[web:n]]` / pack alias `web:n`.
    pub web_index: usize,
    /// Which sub-queries contributed this URL (after normalize).
    pub source_queries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergedWebHits {
    pub queries: Vec<String>,
    pub hits: Vec<MergedWebHit>,
}

/// Normalize URL for dedupe (lowercase host path; strip trailing slash / fragment).
pub fn normalize_url_key(url: &str) -> String {
    let u = url.trim();
    if u.is_empty() {
        return String::new();
    }
    // Lightweight: no full URL parse dependency — strip fragment, lower-case, trim trailing /.
    let without_frag = u.split('#').next().unwrap_or(u);
    let mut s = without_frag.trim().to_ascii_lowercase();
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s
}

/// Merge ordered search responses (one per query). First-seen URL wins; later snippets
/// only fill if the winner snippet is shorter than `min_snippet_keep` chars.
pub fn merge_search_responses(
    pairs: &[(String, SearchResponse)],
    min_snippet_keep: usize,
) -> MergedWebHits {
    let queries: Vec<String> = pairs.iter().map(|(q, _)| q.clone()).collect();
    let mut by_key: Vec<(String, MergedWebHit)> = Vec::new();

    for (query, resp) in pairs {
        for r in &resp.results {
            let key = normalize_url_key(&r.url);
            if key.is_empty() {
                continue;
            }
            if let Some((_, existing)) = by_key.iter_mut().find(|(k, _)| k == &key) {
                if !existing.source_queries.iter().any(|q| q == query) {
                    existing.source_queries.push(query.clone());
                }
                if existing.snippet.chars().count() < min_snippet_keep
                    && r.snippet.chars().count() > existing.snippet.chars().count()
                {
                    existing.snippet = r.snippet.clone();
                    if existing.title.trim().is_empty() {
                        existing.title = r.title.clone();
                    }
                }
            } else {
                by_key.push((
                    key,
                    MergedWebHit {
                        title: r.title.clone(),
                        url: r.url.clone(),
                        snippet: r.snippet.clone(),
                        web_index: 0, // assigned below
                        source_queries: vec![query.clone()],
                    },
                ));
            }
        }
    }

    let mut hits: Vec<MergedWebHit> = by_key.into_iter().map(|(_, h)| h).collect();
    for (i, h) in hits.iter_mut().enumerate() {
        h.web_index = i + 1;
    }

    MergedWebHits { queries, hits }
}

/// Convenience: merge raw result lists with synthetic queries `q0..`.
pub fn merge_web_results(batches: &[Vec<SearchResult>]) -> MergedWebHits {
    let pairs: Vec<(String, SearchResponse)> = batches
        .iter()
        .enumerate()
        .map(|(i, results)| {
            (
                format!("q{i}"),
                SearchResponse {
                    query_type: "web".into(),
                    sub_queries: vec![format!("q{i}")],
                    results: results.clone(),
                    synthesized_answer: String::new(),
                    llm_usage: None,
                },
            )
        })
        .collect();
    merge_search_responses(&pairs, 80)
}

/// Map merged hits into evidence items for an EvidencePack (`channel=web`).
pub fn hits_to_evidence_items(merged: &MergedWebHits) -> Vec<super::evidence_pack::EvidenceItem> {
    merged
        .hits
        .iter()
        .map(|h| super::evidence_pack::EvidenceItem {
            content: if h.snippet.trim().is_empty() {
                h.title.clone()
            } else {
                h.snippet.clone()
            },
            source: h.url.clone(),
            score: 0.0,
            provenance: h.title.clone(),
            alias: format!("web:{}", h.web_index),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(query: &str, results: Vec<(&str, &str, &str)>) -> SearchResponse {
        SearchResponse {
            query_type: "web".into(),
            sub_queries: vec![query.into()],
            results: results
                .into_iter()
                .map(|(t, u, s)| SearchResult {
                    title: t.into(),
                    url: u.into(),
                    snippet: s.into(),
                    citation_index: None,
                })
                .collect(),
            synthesized_answer: String::new(),
            llm_usage: None,
        }
    }

    #[test]
    fn dedupes_url_case_and_slash() {
        let a = resp(
            "q1",
            vec![("T1", "https://Example.com/Path/", "short")],
        );
        let b = resp(
            "q2",
            vec![("T2", "https://example.com/path", "a much longer snippet body for keep")],
        );
        let m = merge_search_responses(
            &[("q1".into(), a), ("q2".into(), b)],
            80,
        );
        assert_eq!(m.hits.len(), 1);
        assert_eq!(m.hits[0].web_index, 1);
        assert_eq!(m.hits[0].source_queries.len(), 2);
        // short < 80 → upgrade snippet from second
        assert!(m.hits[0].snippet.contains("much longer"));
    }

    #[test]
    fn preserves_order_first_seen() {
        let a = resp("a", vec![("A", "https://a.example/1", "aa")]);
        let b = resp(
            "b",
            vec![
                ("B", "https://b.example/2", "bb"),
                ("A2", "https://a.example/1", "ignored"),
            ],
        );
        let m = merge_search_responses(&[("a".into(), a), ("b".into(), b)], 10);
        assert_eq!(m.hits.len(), 2);
        assert_eq!(m.hits[0].url, "https://a.example/1");
        assert_eq!(m.hits[1].url, "https://b.example/2");
        assert_eq!(m.hits[0].web_index, 1);
        assert_eq!(m.hits[1].web_index, 2);
    }

    #[test]
    fn hits_to_evidence_alias() {
        let a = resp("q", vec![("Title", "https://x.test/y", "body")]);
        let m = merge_search_responses(&[("q".into(), a)], 0);
        let items = hits_to_evidence_items(&m);
        assert_eq!(items[0].alias, "web:1");
        assert_eq!(items[0].source, "https://x.test/y");
    }

    #[test]
    fn empty_inputs() {
        let m = merge_search_responses(&[], 80);
        assert!(m.hits.is_empty());
        assert!(m.queries.is_empty());
    }
}
