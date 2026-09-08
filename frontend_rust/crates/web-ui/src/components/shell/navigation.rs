use super::{AccountMenu, NotificationBell};
use crate::i18n::use_i18n;
use crate::routes::dest;
use leptos::prelude::*;

/// Layout state only; page owners retain conversation data and navigation actions.
#[derive(Clone, Copy)]
pub struct NavigationState {
    pub open: RwSignal<bool>,
    pub collapsed: RwSignal<bool>,
}

impl NavigationState {
    pub fn new() -> Self {
        let state = Self { open: RwSignal::new(false), collapsed: RwSignal::new(false) };
        Effect::new(move |_| {
            #[cfg(target_arch = "wasm32")]
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                state.collapsed.set(storage.get_item("context-os.navigation.collapsed").ok().flatten().as_deref() == Some("true"));
            }
        });
        #[cfg(target_arch = "wasm32")]
        {
            let listener = window_event_listener(leptos::ev::resize, move |_| state.open.set(false));
            on_cleanup(move || listener.remove());
        }
        state
    }

    pub fn close(self) {
        self.open.set(false);
        request_animation_frame(move || focus_id("navigation-toggle"));
    }

    fn toggle(self) {
        #[cfg(target_arch = "wasm32")]
        {
            let narrow = web_sys::window().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).is_some_and(|w| w < 1200.0);
            if narrow {
                if self.open.get_untracked() { self.close(); }
                else { self.open.set(true); request_animation_frame(move || focus_id("navigation-collapse")); }
                return;
            }
        }
        self.collapsed.update(|value| *value = !*value);
        #[cfg(target_arch = "wasm32")]
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item("context-os.navigation.collapsed", &self.collapsed.get_untracked().to_string());
        }
    }
}

fn focus_id(id: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        if let Some(el) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(id)).and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) { let _ = el.focus(); }
    }
    let _ = id;
}

fn trap_navigation(event: leptos::ev::KeyboardEvent, state: NavigationState) {
    if !state.open.get_untracked() || event.default_prevented() { return; }
    if event.key() == "Escape" { event.prevent_default(); state.close(); return; }
    #[cfg(target_arch = "wasm32")]
    if event.key() == "Tab" {
        use wasm_bindgen::JsCast;
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return; };
        let Some(rail) = doc.get_element_by_id("chat-session-drawer") else { return; };
        let Ok(nodes) = rail.query_selector_all("button:not([disabled]), a[href], input:not([disabled])") else { return; };
        let items: Vec<web_sys::HtmlElement> = (0..nodes.length()).filter_map(|i| nodes.item(i)).filter_map(|node| node.dyn_into().ok()).filter(|el: &web_sys::HtmlElement| el.offset_width() > 0).collect();
        let (Some(first), Some(last)) = (items.first(), items.last()) else { return; };
        if let Some(active) = doc.active_element() {
            if event.shift_key() && active == **first { event.prevent_default(); let _ = last.focus(); }
            else if !event.shift_key() && active == **last { event.prevent_default(); let _ = first.focus(); }
        }
    }
}

#[component]
fn PanelIcon() -> impl IntoView {
    view! { <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="M9 4v16"/></svg> }
}

#[component]
pub fn NavigationRail(state: NavigationState, children: Children) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <button class="app-navigation-dismiss" data-testid="chat-rail-dismiss" hidden=move || !state.open.get()
            aria-label=move || i18n.t("chat.sessionsDismiss") tabindex="-1" on:click=move |_| state.close()/>
        <aside class="app-navigation" id="chat-session-drawer" data-testid="session-list"
            aria-label=move || i18n.t("chat.sessionsListLabel") on:keydown=move |ev| trap_navigation(ev, state)>
            <div class="app-navigation-head">
                <a href=dest::CHAT class="app-navigation-brand" data-testid="app-topbar-brand">"Context-OS"</a>
                <button type="button" class="app-icon-button" id="navigation-collapse" data-testid="navigation-collapse"
                    aria-label=move || i18n.t("chat.sessionsToggle") on:click=move |_| state.toggle()><PanelIcon/></button>
            </div>
            <nav class="app-navigation-shortcuts" aria-label=move || i18n.t("chat.workspaces")>
                <a class="app-icon-button" href=dest::CHAT title=move || i18n.t("chat.newConversation") aria-label=move || i18n.t("chat.newConversation")>
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
                </a>
                <a class="app-icon-button" href=dest::DASHBOARD title=move || i18n.t("chat.workspaces") aria-label=move || i18n.t("chat.workspaces")>
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M3 7V4h7l2 3h9v13H3z"/></svg>
                </a>
            </nav>
            <div class="app-navigation-content">{children()}</div>
            <div class="app-navigation-account"><AccountMenu/></div>
        </aside>
    }
}

#[component]
pub fn ContextTopBar(state: NavigationState, title: Signal<String>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <header class="app-context-bar" data-testid="app-top-bar">
            <button type="button" id="navigation-toggle" class="app-icon-button app-navigation-toggle" data-testid="chat-rail-toggle"
                aria-controls="chat-session-drawer" aria-expanded=move || state.open.get()
                aria-label=move || i18n.t("chat.sessionsToggle") on:click=move |_| state.toggle()><PanelIcon/></button>
            <span class="app-context-title">{move || title.get()}</span>
            <NotificationBell/>
        </header>
    }
}
