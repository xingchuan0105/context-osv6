use crate::routes::dest;
use leptos::prelude::*;

#[component]
pub fn ShareAccessMenu() -> impl IntoView {
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
                "分享 ▾"
            </button>
            <Show when=move || open.get()>
                <button
                    type="button"
                    class="app-menu-dismiss"
                    aria-label="关闭菜单"
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
                        "访问"
                    </a>
                    <a
                        class="app-menu-item"
                        href=dest::API_ACCESS
                        role="menuitem"
                        data-testid="app-topbar-share-api"
                        on:click=move |_| open.set(false)
                    >
                        "API"
                    </a>
                    <a
                        class="app-menu-item"
                        href=dest::PRICING
                        role="menuitem"
                        data-testid="app-topbar-share-upgrade"
                        on:click=move |_| open.set(false)
                    >
                        "升级"
                    </a>
                </div>
            </Show>
        </div>
    }
}
