//! Home page

use leptos::prelude::*;
use leptos_router::components::Redirect;

use crate::i18n::choose;
use crate::state::ui_prefs::use_ui_prefs_state;

/// Home page component - redirects to /dashboard
#[component]
pub fn HomePage() -> impl IntoView {
    let locale = use_ui_prefs_state().locale;

    view! {
        <>
            <Redirect path="/dashboard"/>
            <div class="min-h-screen flex items-center justify-center bg-background text-muted-foreground">
                {move || choose(locale.get(), "正在跳转...", "Redirecting...")}
            </div>
        </>
    }
}
