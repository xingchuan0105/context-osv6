use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn ShareNavigation(workspace_id: Signal<String>, selected: &'static str) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <nav class="settings-nav" aria-label=move || i18n.t("share.navLabel")>
            {[("", "share.linkSettings"), ("/access-logs", "share.logsTitle"), ("/analytics", "share.analyticsNav")].into_iter().map(|(suffix, label)| view! {
                <a class=if selected == suffix { "settings-nav-item is-active" } else { "settings-nav-item" }
                    aria-current=(selected == suffix).then_some("page")
                    href=move || format!("/dashboard/{}/share{suffix}", workspace_id.get())>
                    {move || i18n.t(label)}
                </a>
            }).collect_view()}
        </nav>
    }
}
