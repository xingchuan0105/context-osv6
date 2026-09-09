use crate::i18n::use_i18n;
use leptos::prelude::*;

/// One persistent content tree: a nonmodal desktop panel and a native modal drawer on narrow screens.
#[component]
pub fn SidePanel(open: RwSignal<bool>, title: Signal<String>, children: Children) -> impl IntoView {
    let i18n = use_i18n();
    let host = NodeRef::<leptos::html::Dialog>::new();
    let narrow = RwSignal::new(false);
    #[cfg(target_arch = "wasm32")]
    {
        let measure = move || narrow.set(web_sys::window().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).is_some_and(|w| w < 1200.0));
        Effect::new(move |_| measure());
        let listener = window_event_listener(leptos::ev::resize, move |_| measure());
        on_cleanup(move || listener.remove());
    }
    let modal = RwSignal::new(false);
    Effect::new(move |_| {
        let visible = open.get();
        let small = narrow.get();
        #[cfg(target_arch = "wasm32")]
        if let Some(dialog) = host.get() {
            if dialog.open() && (!visible || small != modal.get_untracked()) { dialog.close(); }
            if visible && !dialog.open() {
                if small { let _ = dialog.show_modal(); } else { let _ = dialog.show(); }
                modal.set(small);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (visible, small, modal);
    });
    view! {
        <dialog class="app-side-panel" node_ref=host data-testid="workspace-side-rail" aria-label=move || title.get()
            on:cancel=move |ev: leptos::ev::Event| { ev.prevent_default(); open.set(false); }
            on:keydown=move |ev| { if ev.key() == "Escape" && !ev.default_prevented() { ev.prevent_default(); open.set(false); } }
            on:click=move |ev| {
                #[cfg(target_arch = "wasm32")]
                if let Some(dialog) = host.get_untracked() {
                    let rect = dialog.get_bounding_client_rect();
                    let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
                    if x < rect.left() || x > rect.right() || y < rect.top() || y > rect.bottom() { open.set(false); }
                }
                #[cfg(not(target_arch = "wasm32"))]
                let _ = ev;
            }>
            <header class="app-side-panel-header"><h2>{move || title.get()}</h2><button type="button" class="app-icon-button" data-testid="workspace-panel-close" aria-label=move || i18n.t("appModal.close") on:click=move |_| open.set(false)>"×"</button></header>
            {children()}
        </dialog>
    }
}
