//! Chinese-aware tokenization for PostgreSQL full-text search (`simple` config + space-separated tokens).

use std::sync::OnceLock;

use jieba_rs::Jieba;

static JIEBA: OnceLock<Jieba> = OnceLock::new();

fn jieba() -> &'static Jieba {
    JIEBA.get_or_init(Jieba::new)
}

/// Segment text for FTS indexing / querying (jieba for CJK, whitespace for Latin).
pub fn segment_for_fts(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    jieba().cut(trimmed, false).join(" ")
}

/// Build stored search tokens from user content and optional resolved query.
pub fn merge_search_tokens(content: &str, resolved_query: Option<&str>) -> String {
    let mut tokens = segment_for_fts(content);
    if let Some(resolved) = resolved_query.map(str::trim).filter(|s| !s.is_empty()) {
        let resolved_tokens = segment_for_fts(resolved);
        if !resolved_tokens.is_empty() {
            if tokens.is_empty() {
                tokens = resolved_tokens;
            } else {
                tokens.push(' ');
                tokens.push_str(&resolved_tokens);
            }
        }
    }
    tokens.trim().to_string()
}

/// Merge ranked hit lists with reciprocal rank fusion (RRF).
pub fn rrf_merge<T, F>(
    lists: &[&[T]],
    key: F,
    limit: usize,
) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> i64,
{
    const K: f64 = 60.0;
    let mut scores: std::collections::HashMap<i64, f64> = std::collections::HashMap::new();
    let mut items: std::collections::HashMap<i64, T> = std::collections::HashMap::new();

    for list in lists {
        for (rank, item) in list.iter().enumerate() {
            let id = key(item);
            scores.insert(
                id,
                scores.get(&id).copied().unwrap_or(0.0) + 1.0 / (K + rank as f64 + 1.0),
            );
            items.entry(id).or_insert_with(|| item.clone());
        }
    }

    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    ranked
        .into_iter()
        .take(limit)
        .map(|(id, _)| items.remove(&id).expect("score id must exist in items"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_for_fts_splits_chinese() {
        let segmented = segment_for_fts("反脆弱性是什么");
        assert!(segmented.contains(' '));
        assert!(segmented.split_whitespace().count() >= 2);
    }

    #[test]
    fn merge_search_tokens_combines_content_and_resolved() {
        let tokens = merge_search_tokens("Who wrote it?", Some("Who wrote Antifragile?"));
        assert!(!tokens.is_empty());
    }

    #[test]
    fn rrf_merge_prefers_items_in_multiple_lists() {
        #[derive(Clone)]
        struct Hit {
            id: i64,
        }
        let a = [Hit { id: 1 }, Hit { id: 2 }];
        let b = [Hit { id: 2 }, Hit { id: 3 }];
        let merged = rrf_merge(&[&a, &b], |h| h.id, 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].id, 2);
    }
}
