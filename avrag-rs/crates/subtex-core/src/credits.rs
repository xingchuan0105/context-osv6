//! Convert usage quantities into millicredits (F7).
//!
//! 1000 millicredits = 1 credit. The Agent-facing tools never mention this
//! unit; the global ledger is the A-line book, not a wallet UI.

/// Millicredits for one usage row. Unknown kinds yield 0 (no credit line).
pub fn millicredits_for(kind: &str, quantity: f64, unit: Option<&str>) -> i64 {
    let quantity = if quantity.is_finite() && quantity > 0.0 {
        quantity
    } else {
        return 0;
    };
    match (kind, unit.unwrap_or("")) {
        ("transcription", "seconds") => (quantity * 1000.0).round() as i64,
        ("embedding", "tokens") => quantity.round() as i64,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcription_seconds_are_milli() {
        assert_eq!(millicredits_for("transcription", 30.0, Some("seconds")), 30_000);
        assert_eq!(millicredits_for("transcription", 0.0, Some("seconds")), 0);
    }

    #[test]
    fn embedding_tokens_are_one_milli_each() {
        assert_eq!(millicredits_for("embedding", 1500.0, Some("tokens")), 1500);
    }

    #[test]
    fn unknown_kind_is_zero() {
        assert_eq!(millicredits_for("rerank", 3.0, Some("tokens")), 0);
        assert_eq!(millicredits_for("embedding", 10.0, Some("seconds")), 0);
    }
}
