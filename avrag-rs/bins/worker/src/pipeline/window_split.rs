//! Even-partition document text by context-window budget (design 2026-08-06).

use super::helpers::estimate_token_count;

/// Official context window C (tokens). Env `INGESTION_LLM_CONTEXT_WINDOW_TOKENS`.
pub(crate) fn context_window_tokens() -> i64 {
    std::env::var("INGESTION_LLM_CONTEXT_WINDOW_TOKENS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1_000_000)
}

/// Utilization u in (0,1]; default 0.8. Env `INGESTION_LLM_WINDOW_UTILIZATION`.
pub(crate) fn window_utilization() -> f64 {
    std::env::var("INGESTION_LLM_WINDOW_UTILIZATION")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0 && *v <= 1.0)
        .unwrap_or(0.8)
}

/// K = floor(u * C). Body+hint envelope for one window.
pub(crate) fn window_token_budget() -> i64 {
    let c = context_window_tokens();
    let u = window_utilization();
    ((c as f64) * u).floor().max(1.0) as i64
}

/// Minimum N such that T/N < K (integer: N = max(1, ceil(T/K))).
pub(crate) fn window_count(total_tokens: i64, budget_k: i64) -> usize {
    let t = total_tokens.max(0);
    let k = budget_k.max(1);
    if t == 0 {
        return 1;
    }
    let n = (t + k - 1) / k;
    n.max(1) as usize
}

/// Default overlap between adjacent windows (fraction of each core span length).
pub(crate) const WINDOW_OVERLAP_RATIO: f64 = 0.10;

/// Core budget so that inject size ≈ core × (1 + 2×overlap) stays ≤ K.
/// End windows only extend one side (~1+ratio); use worst-case 1+2×ratio for all cores.
pub(crate) fn core_token_budget(inject_k: i64, overlap_ratio: f64) -> i64 {
    let ratio = overlap_ratio.clamp(0.0, 0.45);
    let factor = 1.0 + 2.0 * ratio;
    ((inject_k as f64) / factor).floor().max(1.0) as i64
}

/// Evenly partition `text` into `n` **non-overlapping** core spans
/// (char length + paragraph-aware snap). Not pack-full.
pub(crate) fn even_partition(text: &str, n: usize) -> Vec<String> {
    even_partition_cores(text, n)
        .into_iter()
        .map(|(_s, _e, s)| s)
        .collect()
}

/// Core spans as (start, end, text) in char indices.
fn even_partition_cores(text: &str, n: usize) -> Vec<(usize, usize, String)> {
    let n = n.max(1);
    if n == 1 || text.is_empty() {
        return vec![(0, text.chars().count(), text.to_string())];
    }
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len == 0 {
        return vec![(0, 0, String::new()); n];
    }
    let mut cores = Vec::with_capacity(n);
    let mut start = 0usize;
    for i in 0..n {
        if start >= len {
            cores.push((len, len, String::new()));
            continue;
        }
        let ideal_end = if i + 1 == n {
            len
        } else {
            ((i + 1) * len) / n
        };
        let end = if i + 1 == n {
            len
        } else {
            snap_partition_boundary(&chars, start, ideal_end, len)
        };
        let end = end.max(start + 1).min(len);
        cores.push((start, end, chars[start..end].iter().collect()));
        start = end;
    }
    cores
}

/// Even partition into `n` windows with **overlap** on inject: each window
/// re-includes ~`overlap_ratio` of the neighboring core length (default 10%).
pub(crate) fn even_partition_with_overlap(
    text: &str,
    n: usize,
    overlap_ratio: f64,
) -> Vec<String> {
    let n = n.max(1);
    let ratio = overlap_ratio.clamp(0.0, 0.45);
    if n == 1 || text.is_empty() || ratio == 0.0 {
        return even_partition(text, n);
    }
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let cores = even_partition_cores(text, n);
    let mut parts = Vec::with_capacity(n);
    for (i, &(c_start, c_end, _)) in cores.iter().enumerate() {
        let core_len = c_end.saturating_sub(c_start);
        let extend = ((core_len as f64) * ratio).round() as usize;
        let mut win_start = c_start;
        let mut win_end = c_end;
        if i > 0 && extend > 0 {
            win_start = c_start.saturating_sub(extend);
        }
        if i + 1 < n && extend > 0 {
            win_end = (c_end + extend).min(len);
        }
        // Soft-snap to nearby newlines so overlap does not start mid-glyph/line.
        if win_start > 0 && win_start < c_start {
            let lo = win_start.saturating_sub(24);
            for j in (lo..=win_start).rev() {
                if j > 0 && chars.get(j - 1) == Some(&'\n') {
                    win_start = j;
                    break;
                }
            }
        }
        if win_end > c_end && win_end < len {
            let hi = (win_end + 24).min(len);
            for j in win_end..hi {
                if chars.get(j) == Some(&'\n') {
                    win_end = j + 1;
                    break;
                }
            }
        }
        win_start = win_start.min(win_end);
        parts.push(chars[win_start..win_end].iter().collect());
    }
    parts
}

fn snap_partition_boundary(chars: &[char], start: usize, ideal: usize, len: usize) -> usize {
    let ideal = ideal.clamp(start + 1, len);
    // Prefer double-newline, then single newline, within a small window around ideal.
    let window = (len / 50).clamp(32, 512);
    let lo = ideal.saturating_sub(window).max(start + 1);
    let hi = (ideal + window).min(len);
    // Search backward from ideal for paragraph break.
    let best = ideal;
    let mut i = ideal;
    while i > lo {
        if i >= 2 && chars[i - 1] == '\n' && chars[i - 2] == '\n' {
            return i;
        }
        i -= 1;
    }
    i = ideal;
    while i > lo {
        if chars[i - 1] == '\n' {
            return i;
        }
        i -= 1;
    }
    // Forward
    i = ideal;
    while i < hi {
        if i >= 1 && i + 1 < len && chars[i] == '\n' && chars[i + 1] == '\n' {
            return i + 2;
        }
        if chars[i] == '\n' {
            return i + 1;
        }
        i += 1;
    }
    best
}

/// Split raw document text into even windows under inject budget K, with 10% overlap.
///
/// N is sized from **core** budget `K / (1+2r)` so that after overlap each inject
/// window stays ≈ ≤ K (design review: do not size N on non-overlap cores only).
pub(crate) fn split_document_windows(raw_text: &str) -> Vec<String> {
    let k_inject = window_token_budget();
    let k_core = core_token_budget(k_inject, WINDOW_OVERLAP_RATIO);
    let t = estimate_token_count(raw_text).max(1);
    let mut n = window_count(t, k_core);
    // Grow N if any inject still exceeds K (snap/estimate noise).
    for _ in 0..8 {
        let parts = even_partition_with_overlap(raw_text, n, WINDOW_OVERLAP_RATIO);
        let over = parts
            .iter()
            .any(|p| estimate_token_count(p) > k_inject);
        if !over {
            return parts;
        }
        n = n.saturating_add(1);
    }
    even_partition_with_overlap(raw_text, n, WINDOW_OVERLAP_RATIO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_count_single_when_under_budget() {
        assert_eq!(window_count(100, 1000), 1);
        assert_eq!(window_count(1000, 1000), 1);
    }

    #[test]
    fn window_count_ceil_div() {
        assert_eq!(window_count(1001, 1000), 2);
        assert_eq!(window_count(2500, 1000), 3);
    }

    #[test]
    fn even_partition_n1_identity() {
        let t = "hello\n\nworld";
        assert_eq!(even_partition(t, 1), vec![t.to_string()]);
    }

    #[test]
    fn even_partition_two_parts_cover() {
        let t = "aaaa\n\nbbbb\n\ncccc\n\ndddd";
        let parts = even_partition(t, 2);
        assert_eq!(parts.len(), 2);
        assert_eq!(format!("{}{}", parts[0], parts[1]), t);
        assert!(!parts[0].is_empty());
        assert!(!parts[1].is_empty());
    }

    #[test]
    fn budget_default_positive() {
        assert!(window_token_budget() > 0);
    }

    #[test]
    fn overlap_windows_share_boundary_content() {
        // Distinct cores so overlap is detectable.
        let t = "AAAA\n\nBBBB\n\nCCCC\n\nDDDD\n\nEEEE\n\nFFFF";
        let cores = even_partition(t, 2);
        let with_ov = even_partition_with_overlap(t, 2, 0.10);
        assert_eq!(with_ov.len(), 2);
        // Each overlapped window is at least as long as its core.
        assert!(with_ov[0].len() >= cores[0].len());
        assert!(with_ov[1].len() >= cores[1].len());
        // Overlap: end of first and start of second share some content.
        let a = &with_ov[0];
        let b = &with_ov[1];
        let tail = &a[a.len().saturating_sub(a.len() / 5)..];
        assert!(
            b.contains(tail.trim()) || a.contains(&b[..b.len().min(8)]),
            "expected shared region between adjacent windows"
        );
    }

    #[test]
    fn core_budget_accounts_for_two_sided_overlap() {
        // inject K=1000, r=0.1 → core ≈ 1000/1.2 ≈ 833
        assert_eq!(core_token_budget(1000, 0.10), 833);
        assert_eq!(core_token_budget(1000, 0.0), 1000);
    }

    #[test]
    fn inject_windows_respect_token_budget_with_overlap() {
        // Build a long-ish doc so N>1 under a tiny synthetic budget path:
        // use even_partition_with_overlap + core sizing like split_document_windows.
        let para = "汉字测试段落内容用于均分窗口。".repeat(40);
        let t = (0..20)
            .map(|i| format!("## 节{i}\n\n{para}\n\n"))
            .collect::<String>();
        let k_inject = 500i64;
        let k_core = core_token_budget(k_inject, WINDOW_OVERLAP_RATIO);
        let est = estimate_token_count(&t).max(1);
        let n = window_count(est, k_core);
        assert!(n >= 2, "fixture should need multiple windows, n={n}");
        let parts = even_partition_with_overlap(&t, n, WINDOW_OVERLAP_RATIO);
        for (i, p) in parts.iter().enumerate() {
            let tok = estimate_token_count(p);
            assert!(
                tok <= k_inject + 32,
                "window {i} tokens {tok} exceed inject budget {k_inject}"
            );
        }
    }
}
