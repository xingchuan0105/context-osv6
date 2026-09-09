use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn PublicLayout(children: Children) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="public-layout">
            <header class="public-layout-header">
                <a href="/chat" class="app-navigation-brand">"Context-OS"</a>
                <a href="/chat">{move || i18n.t("navigation.chat")}</a>
            </header>
            <div class="public-layout-content">{children()}</div>
        </div>
    }
}
