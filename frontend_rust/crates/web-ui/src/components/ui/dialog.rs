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
    view! {
        <div class="app-dialog-backdrop" hidden=move || !open.get()>
            <div
                class="app-dialog"
                role="dialog"
                aria-label=move || i18n.t(title_key)
                data-testid=test_id
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
            </div>
        </div>
    }
}
