use app_bootstrap::new_memory;
use app_core::AppConfig;

#[test]
fn memory_bootstrap_uses_memory_runtime() {
    let config = AppConfig::default();
    let bootstrap = new_memory(config);

    // `new_memory` wires in-memory adapters for every port (m4-w4d), so the
    // surviving contract is the runtime mode + same-owner chat context —
    // not the presence/absence of individual ports.
    assert!(bootstrap.storage.uses_memory_adapters());
    assert_eq!(bootstrap.storage.runtime_mode(), "memory");
    assert!(bootstrap.chat.uses_memory_adapters());
    assert_eq!(bootstrap.chat.auth.actor_id(), bootstrap.auth.actor_id());
}