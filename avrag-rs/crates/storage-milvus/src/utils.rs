use serde_json::Value;
use uuid::Uuid;

pub fn uuid_field(row: &Value, key: &str) -> anyhow::Result<Uuid> {
    let s = row
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing field {}", key))?;
    Uuid::parse_str(s).map_err(|e| anyhow::anyhow!("invalid uuid in field {}: {}", key, e))
}

pub fn optional_uuid_field(row: &Value, key: &str) -> anyhow::Result<Option<Uuid>> {
    let s = match row.get(key).and_then(Value::as_str) {
        Some(s) => s,
        None => return Ok(None),
    };
    if s.is_empty() {
        return Ok(None);
    }
    // Gracefully ignore non-UUID values (e.g. legacy string identifiers)
    Ok(Uuid::parse_str(s).ok())
}

pub fn string_field(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

/// Convert a Milvus `distance` value into a similarity score in "higher is
/// better" space, honoring the collection's distance metric:
/// - `COSINE` / `IP` (default): pass the distance through unchanged.
/// - `L2`: map distance `d` to similarity `1.0 / (1.0 + d)`, so a lower
///   distance yields a higher score and the result stays in `(0.0, 1.0]`.
/// - unknown metrics: fall back to COSINE-style pass-through.
///
/// `metric_type` is matched case-insensitively. BM25 rows carry a sentinel
/// `"BM25"` metric from `scored_text_chunk` since their `distance` is already a
/// relevance score (not a geometric distance) and must not be inverted.
pub fn score_field(row: &Value, metric_type: &str) -> f32 {
    let distance = row
        .get("distance")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    match metric_type.trim().to_ascii_uppercase().as_str() {
        "L2" => (1.0 / (1.0 + distance)) as f32,
        // COSINE, IP, BM25 (already a relevance score), and any unknown metric:
        // pass the distance through unchanged (higher = better).
        _ => distance as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::score_field;
    use serde_json::json;

    #[test]
    fn cosine_passes_distance_through() {
        // COSINE: higher distance (similarity) is better; returned unchanged.
        let row = json!({ "distance": 0.8 });
        assert!((score_field(&row, "COSINE") - 0.8).abs() < 1e-6);
        // Case-insensitive.
        assert!((score_field(&row, "cosine") - 0.8).abs() < 1e-6);
    }

    #[test]
    fn ip_passes_distance_through() {
        // IP (inner product): higher is better; treated like COSINE.
        let row = json!({ "distance": 0.75 });
        assert!((score_field(&row, "IP") - 0.75).abs() < 1e-6);
    }

    #[test]
    fn l2_inverts_distance() {
        // L2: lower distance is better -> mapped to 1/(1+d), so score rises as
        // distance falls and stays within (0, 1].
        let row = json!({ "distance": 0.5 });
        assert!((score_field(&row, "L2") - (1.0 / 1.5)).abs() < 1e-6);

        // Identical vectors (distance 0) -> perfect score of 1.0.
        let row_zero = json!({ "distance": 0.0 });
        assert!((score_field(&row_zero, "L2") - 1.0).abs() < 1e-6);

        // Case-insensitive.
        assert!((score_field(&row, "l2") - (1.0 / 1.5)).abs() < 1e-6);
    }

    #[test]
    fn l2_orders_correctly_vs_cosine() {
        // Under L2 a nearer neighbor (smaller distance) scores higher.
        let near = json!({ "distance": 0.2 });
        let far = json!({ "distance": 1.0 });
        assert!(score_field(&near, "L2") > score_field(&far, "L2"));
    }

    #[test]
    fn bm25_keeps_raw_relevance() {
        // BM25's distance is a relevance score (higher = better); the sentinel
        // must NOT trigger L2 inversion.
        let row = json!({ "distance": 12.3 });
        assert!((score_field(&row, "BM25") - 12.3).abs() < 1e-6);
    }

    #[test]
    fn unknown_metric_defaults_to_passthrough() {
        let row = json!({ "distance": 0.42 });
        assert!((score_field(&row, "HAMMING") - 0.42).abs() < 1e-6);
    }

    #[test]
    fn missing_distance_defaults_to_zero_distance() {
        // A missing `distance` is treated as 0.0 distance. For COSINE that is a
        // 0.0 score; for L2 it maps to 1/(1+0) = 1.0 (perfect similarity).
        let row = json!({});
        assert_eq!(score_field(&row, "COSINE"), 0.0);
        assert!((score_field(&row, "L2") - 1.0).abs() < 1e-6);
    }
}
