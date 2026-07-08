use storage_local::CachePort;
use tauri::State;

use crate::AppLocalState;

#[tauri::command]
pub async fn get_cache_value(
    state: State<'_, AppLocalState>,
    key: String,
) -> Result<Option<String>, String> {
    Ok(state.cache.get(&key).await)
}

#[tauri::command]
pub async fn set_cache_value(
    state: State<'_, AppLocalState>,
    key: String,
    value: String,
    ttl_secs: u64,
) -> Result<(), String> {
    state.cache.set(&key, &value, ttl_secs).await
}
