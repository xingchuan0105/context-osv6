use app_bootstrap::new_memory;
use app_core::AppConfig;

#[test]
fn memory_bootstrap_has_no_pg_ports() {
    let config = AppConfig::default();
    let bootstrap = new_memory(config);

    assert!(bootstrap.storage.document_store().is_none());
    assert!(bootstrap.storage.admin_store().is_none());
    assert!(bootstrap.storage.billing_quota().is_none());
    assert!(bootstrap.storage.uses_memory_adapters());
    assert_eq!(bootstrap.storage.runtime_mode(), "memory");
    assert_eq!(bootstrap.chat.auth.actor_id(), bootstrap.auth.actor_id());
    assert!(bootstrap.chat.uses_memory_adapters());
}
