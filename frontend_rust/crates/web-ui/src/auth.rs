use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::api_base::poc_api_base;
#[cfg(target_arch = "wasm32")]
use web_sdk::{PersistedAuth, clear_browser_auth, fetch_me, read_browser_auth, write_browser_auth};

/// 从 Next 同键存储恢复 token。无登录框；失败则清存储。
#[component]
pub fn AuthBootstrap() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        bootstrap_from_browser(token);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = token;
        }
    });
    ()
}

#[cfg(target_arch = "wasm32")]
fn bootstrap_from_browser(token: RwSignal<String>) {
    let Some(persisted) = read_browser_auth() else {
        return;
    };
    token.set(persisted.token.clone());
    let base = poc_api_base();
    leptos::task::spawn_local(async move {
        match fetch_me(&base, &persisted.token).await {
            Ok(user) => {
                write_browser_auth(&PersistedAuth {
                    token: persisted.token,
                    user,
                });
            }
            Err(_) => {
                clear_browser_auth();
                token.set(String::new());
            }
        }
    });
}
