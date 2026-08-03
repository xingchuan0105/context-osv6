//! `E2E_ABORT_AFTER_CONSECUTIVE_FAILS` circuit breaker for long eval runs.
//!
//! A systemic break (judge outage, ingest regression, transport wedge) shows
//! up as a long run of consecutive non-PASS labels; running the remaining
//! 100+ questions after that carries no information but costs the full
//! runtime. This breaker trips at a configurable streak so the runner can
//! stop scheduling new questions (in-flight ones still finish — same
//! "weakened break" semantics as `E2E_FAIL_FAST`) and fail loudly.
//!
//! Completions arrive out of order (`buffer_unordered`), so verdicts are
//! settled over the maximal *contiguous* prefix of question numbers: the
//! trailing non-PASS streak is recomputed on every record, which makes
//! inversions harmless. Filtered runs (`E2E_QUESTIONS`) and offset runs
//! (`E2E_START_AT`) simply never build a long contiguous prefix, so the
//! breaker stays inert there — the intended behavior for targeted re-runs.

use std::collections::BTreeMap;

/// Counts the trailing run of consecutive non-PASS verdicts across the
/// contiguous settled question prefix. `threshold == 0` disables.
#[derive(Debug)]
pub struct ConsecutiveNonPassBreaker {
    threshold: usize,
    /// qnum → is_pass for every recorded question.
    settled: BTreeMap<usize, bool>,
    tripped: bool,
}

impl ConsecutiveNonPassBreaker {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            settled: BTreeMap::new(),
            tripped: false,
        }
    }

    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Record one question's verdict. Returns `true` only on the record that
    /// newly trips the breaker (so the caller logs exactly once).
    pub fn record(&mut self, qnum: usize, is_pass: bool) -> bool {
        if self.threshold == 0 || self.tripped {
            return false;
        }
        self.settled.insert(qnum, is_pass);
        // Trailing non-PASS streak over the contiguous settled prefix.
        let mut streak = 0usize;
        let mut expect = None;
        for (&q, &pass) in &self.settled {
            if let Some(e) = expect {
                if q != e {
                    break; // gap: question still in flight or filtered out
                }
            }
            streak = if pass { 0 } else { streak + 1 };
            expect = Some(q + 1);
        }
        if streak >= self.threshold {
            self.tripped = true;
            return true;
        }
        false
    }

    pub fn tripped(&self) -> bool {
        self.tripped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_never_trips() {
        let mut b = ConsecutiveNonPassBreaker::new(0);
        for q in 1..=20 {
            assert!(!b.record(q, false));
        }
        assert!(!b.tripped());
    }

    #[test]
    fn trips_at_threshold_and_reports_once() {
        let mut b = ConsecutiveNonPassBreaker::new(3);
        assert!(!b.record(1, false));
        assert!(!b.record(2, false));
        assert!(b.record(3, false)); // newly tripped
        assert!(b.tripped());
        assert!(!b.record(4, false)); // already tripped: no second signal
    }

    #[test]
    fn pass_resets_streak() {
        let mut b = ConsecutiveNonPassBreaker::new(3);
        b.record(1, false);
        b.record(2, false);
        b.record(3, true); // streak 2 → 0
        b.record(4, false);
        assert!(!b.record(5, false)); // trailing streak 2
        assert!(b.record(6, false)); // q4..q6 = 3 consecutive non-PASS → trip
    }

    #[test]
    fn out_of_order_completion_counts_in_question_order() {
        let mut b = ConsecutiveNonPassBreaker::new(3);
        assert!(!b.record(3, false)); // prefix = [3]: streak 1
        assert!(!b.record(1, false)); // prefix = [1], gap at 2: streak 1
        assert!(b.record(2, false)); // prefix = [1,2,3]: streak 3 → trip
    }

    #[test]
    fn sparse_filtered_run_never_trips() {
        let mut b = ConsecutiveNonPassBreaker::new(3);
        for q in [1, 5, 9, 13] {
            assert!(!b.record(q, false));
        }
        assert!(!b.tripped());
    }

    #[test]
    fn offset_start_trips_on_its_own_prefix() {
        let mut b = ConsecutiveNonPassBreaker::new(2);
        assert!(!b.record(58, false));
        assert!(b.record(59, false));
    }
}
