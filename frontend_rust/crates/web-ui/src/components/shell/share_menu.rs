use crate::i18n::use_i18n;
use crate::routes::dest;
use leptos::prelude::*;

#[component]
pub fn ShareAccessMenu() -> impl IntoView {
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    view! {
        <div class="app-menu">
            <button
                type="button"
                class="app-top-bar-capsule"
                aria-haspopup="menu"
                aria-expanded=move || open.get()
                data-testid="app-topbar-share-menu"
                on:click=move |_| open.update(|v| *v = !*v)
            >
                {move || i18n.t("shareMenu.trigger")}
            </button>
            <Show when=move || open.get()>
                <button
                    type="button"
                    class="app-menu-dismiss"
                    aria-label=move || i18n.t("commonMenuClose")
                    on:click=move |_| open.set(false)
                />
                <div class="app-menu-panel" role="menu" data-testid="app-topbar-share-menu-panel">
                    <a
                        class="app-menu-item"
                        href=dest::SHARE_TRAFFIC
                        role="menuitem"
                        data-testid="app-topbar-share-traffic"
                        on:click=move |_| open.set(false)
                    >
                        {move || i18n.t("shareMenu.access")}
                    </a>
                    <a
                        class="app-menu-item"
                        href=dest::API_ACCESS
                        role="menuitem"
                        data-testid="app-topbar-share-api"
                        on:click=move |_| open.set(false)
                    >
                        {move || i18n.t("workspaceApi")}
                    </a>
                    <a
                        class="app-menu-item"
                        href=dest::PRICING
                        role="menuitem"
                        data-testid="app-topbar-share-upgrade"
                        on:click=move |_| open.set(false)
                    >
                        {move || i18n.t("planEntry.upgrade")}
                    </a>
                </div>
            </Show>
        </div>
    }
}
