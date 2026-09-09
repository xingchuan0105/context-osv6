use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn SettingsNavigation(selected: Signal<String>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <nav class="settings-nav" aria-label=move || i18n.t("settings.navAria")>
            {[("profile", "settings.tabs.profile"), ("providers", "settings.tabs.providers"), ("preferences", "settings.tabs.preferences"), ("billing", "settings.tabs.billing"), ("security", "settings.tabs.security"), ("usage", "settings.usageLink")].into_iter().map(|(id, label)| view! {
                <a class=move || if selected.get() == id { "settings-nav-item is-active" } else { "settings-nav-item" }
                    aria-current=move || (selected.get() == id).then_some("page")
                    data-testid=format!("tab-{id}") href=if id == "usage" { "/settings/usage".to_string() } else { format!("/settings?tab={id}") }>
                    {move || i18n.t(label)}
                </a>
            }).collect_view()}
        </nav>
    }
}
