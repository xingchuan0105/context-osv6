use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
fn cache_key(workspace: &str) -> Option<String> {
    web_sdk::read_browser_auth().map(|auth| format!("context-os.note-draft:{}:{workspace}", auth.user.email))
}

pub fn read(workspace: &str) -> (String, String) {
    #[cfg(target_arch = "wasm32")]
    if let Some(key) = cache_key(workspace) {
        if let Some(value) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()).and_then(|s| s.get_item(&key).ok().flatten()) {
            if let Ok(draft) = serde_json::from_str(&value) { return draft; }
        }
    }
    let _ = workspace;
    (String::new(), String::new())
}

pub fn save(workspace: &str, title: &str, content: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(key) = cache_key(workspace) {
        if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
            if title.is_empty() && content.is_empty() { let _ = storage.remove_item(&key); }
            else if let Ok(value) = serde_json::to_string(&(title, content)) { let _ = storage.set_item(&key, &value); }
        }
    }
    let _ = (workspace, title, content);
}

/// Browser-tab draft recovery accompanies explicit confirmation on links and document unload.
/// Browser back/forward can restore the draft from its user/workspace-scoped cache.
pub fn guard(workspace: Signal<String>, dirty: Signal<bool>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::{closure::Closure, JsCast};
        let unload = window_event_listener(leptos::ev::beforeunload, move |ev| {
            if dirty.get_untracked() { ev.prevent_default(); ev.set_return_value(""); }
        });
        on_cleanup(move || unload.remove());
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return; };
        let click = Closure::wrap(Box::new(move |ev: web_sys::MouseEvent| {
            if !dirty.get_untracked() || ev.button() != 0 || ev.ctrl_key() || ev.meta_key() || ev.shift_key() || ev.alt_key() { return; }
            let Some(link) = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|el| el.closest("a[href]").ok().flatten()) else { return; };
            if link.get_attribute("target").as_deref() == Some("_blank") || link.has_attribute("download") { return; }
            let Some(window) = web_sys::window() else { return; };
            let Some(href) = link.get_attribute("href") else { return; };
            let Ok(base) = window.location().href() else { return; };
            let Ok(url) = web_sys::Url::new_with_base(&href, &base) else { return; };
            if url.pathname() == format!("/dashboard/{}", workspace.get_untracked()) && url.origin() == window.location().origin().unwrap_or_default() { return; }
            if !window.confirm_with_message(&crate::i18n::t_now("workbench.unsavedNote")).unwrap_or(false) {
                ev.prevent_default(); ev.stop_immediate_propagation();
            }
        }) as Box<dyn FnMut(web_sys::MouseEvent)>);
        let _ = doc.add_event_listener_with_callback_and_bool("click", click.as_ref().unchecked_ref(), true);
        let listener = StoredValue::new_local((doc, click));
        on_cleanup(move || { listener.try_with_value(|(doc, click)| { let _ = doc.remove_event_listener_with_callback_and_bool("click", click.as_ref().unchecked_ref(), true); }); });
    }
    let _ = (workspace, dirty);
}
