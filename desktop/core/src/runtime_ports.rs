//! Ports shared by native startup, generated configuration and status probes.
fn port(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value != 0)
        .unwrap_or(default)
}

pub(crate) fn postgres() -> u16 {
    port("CLIENT_PG_PORT", 5433)
}
pub(crate) fn redis() -> u16 {
    port("CLIENT_REDIS_PORT", 6380)
}
pub(crate) fn api() -> u16 {
    port("CLIENT_API_PORT", 18080)
}
