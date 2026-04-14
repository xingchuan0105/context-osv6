use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;

#[cfg(target_arch = "wasm32")]
use gloo_timers::future::TimeoutFuture;

pub fn run_once_after_hydration<K, F>(
    key: K,
    loaded_key: ReadSignal<String>,
    set_loaded_key: WriteSignal<String>,
    on_change: F,
) where
    K: Fn() -> String + 'static,
    F: Fn() + Clone + 'static,
{
    Effect::new(move |_| {
        let next_key = key();
        if next_key.is_empty() || loaded_key.get() == next_key {
            return;
        }
        set_loaded_key.set(next_key);

        #[cfg(target_arch = "wasm32")]
        {
            let on_change = on_change.clone();
            spawn(async move {
                TimeoutFuture::new(0).await;
                on_change();
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            on_change();
        }
    });
}
