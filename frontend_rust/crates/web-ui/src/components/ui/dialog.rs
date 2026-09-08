use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn AppDialog(
    open: Signal<bool>,
    title_key: &'static str,
    test_id: &'static str,
    on_close: Callback<()>,
    children: Children,
) -> impl IntoView {
    let i18n = use_i18n();
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    Effect::new(move |_| {
        let visible = open.get();
        #[cfg(target_arch = "wasm32")]
        if let Some(element) = dialog.get() {
            if visible && !element.open() { let _ = element.show_modal(); }
            if !visible && element.open() { element.close(); }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = visible;
    });
    view! {
            <dialog
                node_ref=dialog
                class="app-dialog"
                aria-label=move || i18n.t(title_key)
                data-testid=test_id
                on:cancel=move |event: leptos::ev::Event| { event.prevent_default(); on_close.run(()); }
            >
                <header class="app-dialog-header">
                    <h2>{move || i18n.t(title_key)}</h2>
                    <button
                        type="button"
                        class="app-dialog-close"
                        data-testid="dialog-close"
                        on:click=move |_| on_close.run(())
                    >
                        {move || i18n.t("appModal.close")}
                    </button>
                </header>
                <div class="app-dialog-body">{children()}</div>
            </dialog>
    }
}
