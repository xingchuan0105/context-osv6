use app::{AppConfig, AppState};

#[tokio::test]
async fn bootstrap_without_database_url_uses_memory_adapters() {
    let state = AppState::bootstrap(AppConfig {
        database_url: None,
        ..AppConfig::default()
    })
    .await
    .unwrap();

    assert_eq!(state.runtime_mode(), "memory");
    assert!(state.uses_memory_adapters());
}
