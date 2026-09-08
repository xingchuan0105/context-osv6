use leptos::prelude::*;

#[component]
pub fn AppDialog(
    open: Signal<bool>,
    title: &'static str,
    test_id: &'static str,
    on_close: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="app-dialog-backdrop" hidden=move || !open.get()>
            <div class="app-dialog" role="dialog" aria-label=title data-testid=test_id>
                <header class="app-dialog-header">
                    <h2>{title}</h2>
                    <button
                        type="button"
                        class="app-dialog-close"
                        data-testid="dialog-close"
                        on:click=move |_| on_close.run(())
                    >
                        "关闭"
                    </button>
                </header>
                <div class="app-dialog-body">{children()}</div>
            </div>
        </div>
    }
}
