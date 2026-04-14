//! Auth state - global reactive auth state using Leptos signals

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use web_sdk::dtos::AuthUserDto;

#[cfg(target_arch = "wasm32")]
const AUTH_STORAGE_KEY: &str = "avrag.auth.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedAuth {
    token: String,
    user: AuthUserDto,
}

#[cfg(target_arch = "wasm32")]
fn read_persisted_auth() -> Option<PersistedAuth> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    let raw = storage.get(AUTH_STORAGE_KEY).ok().flatten()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn read_persisted_auth() -> Option<PersistedAuth> {
    None
}

#[cfg(target_arch = "wasm32")]
fn persist_auth(token: &str, user: &AuthUserDto) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let payload = PersistedAuth {
        token: token.to_string(),
        user: user.clone(),
    };
    if let Ok(raw) = serde_json::to_string(&payload) {
        let _ = storage.set(AUTH_STORAGE_KEY, &raw);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn persist_auth(_token: &str, _user: &AuthUserDto) {}

#[cfg(target_arch = "wasm32")]
fn clear_persisted_auth() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return;
    };
    let _ = storage.delete(AUTH_STORAGE_KEY);
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_persisted_auth() {}

/// Global auth state managed via Leptos signals.
/// Provides reactive token and user data accessible throughout the component tree.
#[derive(Clone)]
pub struct AuthState {
    pub token: ReadSignal<Option<String>>,
    pub user: ReadSignal<Option<AuthUserDto>>,
    set_token: WriteSignal<Option<String>>,
    set_user: WriteSignal<Option<AuthUserDto>>,
}

impl AuthState {
    /// Returns true if a valid token is present.
    pub fn is_authenticated(&self) -> bool {
        self.token.get_untracked().is_some()
    }

    /// Clears token and user, logging the user out.
    pub fn logout(&self) {
        self.set_token.set(None);
        self.set_user.set(None);
        clear_persisted_auth();
    }

    /// Sets the auth token and optionally the user.
    pub fn set_auth(&self, token: String, user: AuthUserDto) {
        self.set_token.set(Some(token));
        self.set_user.set(Some(user));
        if let (Some(token), Some(user)) = (self.token.get_untracked(), self.user.get_untracked()) {
            persist_auth(&token, &user);
        }
    }
}

/// Provides the auth state as a Leptos context.
pub fn provide_auth_state() -> AuthState {
    let persisted = read_persisted_auth();
    let (token, set_token) =
        signal::<Option<String>>(persisted.as_ref().map(|auth| auth.token.clone()));
    let (user, set_user) = signal::<Option<AuthUserDto>>(persisted.map(|auth| auth.user));

    let state = AuthState {
        token,
        user,
        set_token,
        set_user,
    };

    provide_context(state.clone());
    state
}

/// Retrieves the auth state from context.
/// Panics if called outside of a component that has called `provide_auth_state`.
pub fn use_auth_state() -> AuthState {
    use_context().expect("AuthState not provided - did you call provide_auth_state()?")
}
