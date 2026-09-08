use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn PageStatus(
    loading: Signal<bool>,
    error: Signal<Option<String>>,
    empty: Signal<bool>,
    empty_key: &'static str,
    children: Children,
) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <p class="page-status-loading" data-testid="page-loading" hidden=move || !loading.get()>
            {move || i18n.t("commonLoading")}
        </p>
        <p
            class="page-status-error"
            role="alert"
            data-testid="page-error"
            hidden=move || error.get().is_none()
        >
            {move || error.get().unwrap_or_default()}
        </p>
        <p
            class="page-status-empty"
            data-testid="page-empty"
            hidden=move || loading.get() || error.get().is_some() || !empty.get()
        >
            {move || i18n.t(empty_key)}
        </p>
        <div hidden=move || loading.get() || error.get().is_some() || empty.get()>
            {children()}
        </div>
    }
}
