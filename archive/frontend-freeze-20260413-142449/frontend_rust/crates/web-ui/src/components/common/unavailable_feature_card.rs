use leptos::prelude::*;

use crate::i18n::choose;
use crate::state::ui_prefs::use_ui_prefs_state;

#[component]
pub fn UnavailableFeatureCard(title: String, description: String) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;

    view! {
        <div class="app-surface-card">
            <div class="space-y-3">
                <span class="inline-flex rounded-full bg-muted px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                    {move || choose(locale.get(), "功能暂不可用", "Feature unavailable")}
                </span>
                <div class="space-y-2">
                    <h2 class="text-lg font-semibold text-card-foreground">{title}</h2>
                    <p class="text-sm text-muted-foreground">{description}</p>
                </div>
            </div>
        </div>
    }
}
