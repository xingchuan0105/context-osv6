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

pub fn score_field(row: &Value) -> f32 {
    row.get("distance")
        .and_then(Value::as_f64)
        .map(|f| f as f32)
        .unwrap_or(0.0)
}
