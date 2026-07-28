//! K1 (2026-07-28, evidence-plane design §3.1): score-shape adaptive top-k.
//!
//! Deterministic largest-gap algorithm (Taguchi et al., EMNLP 2025) with a
//! flatness guard. Applied at the retrieval tool's EXIT (dense: post-rerank
//! scores; lexical: similarity), it replaces the fixed `dynamic_final_feed`
//! ratio cut — the rough/recall pool upstream is unchanged.
//!
//! ```text
//! input: descending score list s[1..n]
//! w = min(n, 8); gaps[i] = s[i]-s[i+1] (i in 1..w-1); i* = argmax(gaps)
//! range = s[1] - s[w]; flat_thresh = max(0.02, 0.03 * |s[1]|)
//! ① range < flat_thresh      → FlatAllSame           → k = 5
//! ② gaps[i*]/range ≥ 0.4     → Steep                 → k = clamp(i*, 1, 5)
//! ③ else                     → FlatLowDiscrimination → k = 5
//! ```

/// Score distribution shape of one retrieval call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreShape {
    /// A decisive gap inside the window — cut at the gap (k = gap position).
    Steep,
    /// Some spread, but no dominant gap — return the wider flat cut.
    FlatLowDiscrimination,
    /// All candidates ~same score — likely no effective hit at all.
    FlatAllSame,
}

impl ScoreShape {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Steep => "steep",
            Self::FlatLowDiscrimination => "flat_low_discrimination",
            Self::FlatAllSame => "flat_all_same",
        }
    }
}

/// Adaptive cut decision: how many of the top candidates to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveK {
    pub k: usize,
    pub shape: ScoreShape,
}

const WINDOW: usize = 8;
const K_MIN: usize = 1;
const K_MAX: usize = 5;
const FLAT_ABS: f32 = 0.02;
const FLAT_REL: f32 = 0.03;
const GAP_RATIO: f32 = 0.4;

/// Compute the adaptive cut over a DESCENDING score list.
///
/// Contract: callers pass the score set used for display (reranked when
/// available, else raw retrieval scores), best-first. The function tolerates
/// unsorted input by not re-sorting (gap math then reflects the given order).
pub fn adaptive_k(scores: &[f32]) -> AdaptiveK {
    if scores.is_empty() {
        return AdaptiveK {
            k: 0,
            shape: ScoreShape::FlatAllSame,
        };
    }
    let w = scores.len().min(WINDOW);
    if w == 1 {
        // Single candidate: nothing to discriminate — treat as flat, cut is 1.
        return AdaptiveK {
            k: K_MAX.min(scores.len()),
            shape: ScoreShape::FlatAllSame,
        };
    }
    let top = scores[0];
    let range = top - scores[w - 1];
    let flat_thresh = FLAT_ABS.max(FLAT_REL * top.abs());

    // ① mixed flat threshold (absolute + relative — scale-immune).
    if range < flat_thresh {
        return AdaptiveK {
            k: K_MAX.min(scores.len()),
            shape: ScoreShape::FlatAllSame,
        };
    }

    let mut gap_index = 0usize; // gap between s[g] and s[g+1]
    let mut gap_max = f32::MIN;
    for i in 0..(w - 1) {
        let gap = scores[i] - scores[i + 1];
        if gap > gap_max {
            gap_max = gap;
            gap_index = i;
        }
    }

    // ② dominant gap → cut right after it (k = 1-based gap position).
    if gap_max / range >= GAP_RATIO {
        return AdaptiveK {
            k: (gap_index + 1).clamp(K_MIN, K_MAX).min(scores.len()),
            shape: ScoreShape::Steep,
        };
    }

    // ③ no dominant gap.
    AdaptiveK {
        k: K_MAX.min(scores.len()),
        shape: ScoreShape::FlatLowDiscrimination,
    }
}

/// Model-facing coaching hint per arm (design §3.2, 中文).
/// `None` for a steep cut beyond k=2 (nothing to coach).
pub fn hint_text(decision: &AdaptiveK) -> Option<&'static str> {
    match decision.shape {
        ScoreShape::Steep if decision.k <= 2 => Some(
            "命中明确（top 分数梯度大）。可进入分析；若需交叉验证可换角度再查一次。",
        ),
        ScoreShape::Steep => None,
        ScoreShape::FlatLowDiscrimination => Some(
            "结果区分度低（分数平均）。建议：① 换更具体的词（专名/编号/表内字面值）；\
             ② 若换词后仍平均，该语料可能未覆盖——按查无流程处理。",
        ),
        ScoreShape::FlatAllSame => {
            Some("全部候选同分，疑似无有效命中——换检索策略或按查无流程处理。")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(scores: &[f32]) -> AdaptiveK {
        adaptive_k(scores)
    }

    #[test]
    fn steep_gap_at_first_position_cuts_one() {
        // Gap after s[0] dominates the whole range → k=1.
        let d = k(&[0.9, 0.3, 0.29, 0.28, 0.27, 0.26, 0.25, 0.24]);
        assert_eq!(d.shape, ScoreShape::Steep);
        assert_eq!(d.k, 1);
        assert!(hint_text(&d).unwrap().contains("命中明确"));
    }

    #[test]
    fn steep_gap_at_second_position_cuts_two() {
        let d = k(&[0.95, 0.9, 0.4, 0.39, 0.38, 0.37, 0.36, 0.35]);
        assert_eq!(d.shape, ScoreShape::Steep);
        assert_eq!(d.k, 2);
        assert!(hint_text(&d).is_some());
    }

    #[test]
    fn steep_gap_at_fourth_position_cuts_four_without_hint() {
        let d = k(&[0.9, 0.85, 0.8, 0.75, 0.3, 0.29, 0.28, 0.27]);
        assert_eq!(d.shape, ScoreShape::Steep);
        assert_eq!(d.k, 4);
        assert!(hint_text(&d).is_none(), "steep k>2 carries no coaching");
    }

    #[test]
    fn flat_all_same_detected() {
        let d = k(&[0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
        assert_eq!(d.shape, ScoreShape::FlatAllSame);
        assert_eq!(d.k, 5);
        assert!(hint_text(&d).unwrap().contains("全部候选同分"));
    }

    #[test]
    fn low_discrimination_gradual_slope_gives_five() {
        // Even slope: each gap is 1/7 of range < 0.4 → flat cut.
        let d = k(&[0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0]);
        assert_eq!(d.shape, ScoreShape::FlatLowDiscrimination);
        assert_eq!(d.k, 5);
        assert!(hint_text(&d).unwrap().contains("区分度低"));
    }

    #[test]
    fn small_pool_below_window() {
        // n=3 < w: range = s[1]-s[3]; the dominant gap (0.4/0.5) wins → k=1.
        let d = k(&[0.9, 0.5, 0.4]);
        assert_eq!(d.shape, ScoreShape::Steep);
        assert_eq!(d.k, 1);
        // n=3 flat → k capped at n.
        let d = k(&[0.30, 0.30, 0.29]);
        assert_eq!(d.shape, ScoreShape::FlatAllSame);
        assert_eq!(d.k, 3);
    }

    #[test]
    fn single_and_empty_inputs() {
        assert_eq!(k(&[]).k, 0);
        let d = k(&[0.42]);
        assert_eq!(d.k, 1);
        assert_eq!(d.shape, ScoreShape::FlatAllSame);
    }

    #[test]
    fn negative_scores_use_relative_threshold() {
        // Similarity can be negative on some backends; |s1| keeps the
        // threshold scale-relative. Range 0.5 >> thresh → gap test applies.
        let d = k(&[-0.1, -0.6, -0.62, -0.64, -0.66, -0.68, -0.7, -0.72]);
        assert_eq!(d.shape, ScoreShape::Steep);
        assert_eq!(d.k, 1);
        // Tiny absolute spread with negative top → flat.
        let d = k(&[-0.5, -0.5, -0.5]);
        assert_eq!(d.shape, ScoreShape::FlatAllSame);
    }

    #[test]
    fn large_scale_scores_use_relative_threshold() {
        // |s1|=100 → thresh = 3.0; range 2.0 < 3.0 → flat despite big numbers.
        let d = k(&[100.0, 99.0, 98.5, 98.2, 98.1, 98.05, 98.02, 98.01]);
        assert_eq!(d.shape, ScoreShape::FlatAllSame);
    }

    #[test]
    fn gap_below_ratio_is_low_discrimination() {
        // Biggest gap is 0.2/0.7 ≈ 0.29 of range (< 0.4) → no decisive cut.
        let d = k(&[0.9, 0.75, 0.55, 0.4, 0.35, 0.3, 0.25, 0.2]);
        assert_eq!(d.shape, ScoreShape::FlatLowDiscrimination);
        assert_eq!(d.k, 5);
    }
}
