use super::{AccountMenu, NotificationBell, ShareAccessMenu};
use crate::i18n::use_i18n;
use crate::routes::dest;
use leptos::prelude::*;

#[component]
pub fn AppTopBar() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <header class="app-top-bar" data-testid="app-top-bar">
            <a
                class="app-top-bar-brand"
                href=dest::CHAT
                data-testid="app-topbar-brand"
                title=move || i18n.t("chat.newConversation")
            >
                <img
                    class="app-top-bar-mark"
                    src="/brand/context-os-mark.svg"
                    alt=""
                    width="28"
                    height="28"
                />
                <span class="app-top-bar-wordmark">"Context-OS"</span>
            </a>
            <div class="app-top-bar-actions">
                <ShareAccessMenu/>
                <NotificationBell/>
                <AccountMenu/>
            </div>
        </header>
    }
}
