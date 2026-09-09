pub mod account_menu;
pub mod app_top_bar;
pub mod marketing_chrome;
pub mod notification_bell;
pub mod product_chrome_footer;
pub mod share_menu;
pub mod navigation;
pub mod application_layout;
pub use application_layout::ApplicationLayout;

pub use navigation::{ContextTopBar, NavigationRail, NavigationState};

pub use account_menu::AccountMenu;
pub use app_top_bar::AppTopBar;
pub use marketing_chrome::MarketingChrome;
pub use notification_bell::NotificationBell;
pub use product_chrome_footer::ProductChromeFooter;
pub use share_menu::ShareAccessMenu;

use leptos::prelude::*;

/// Product-inner chrome: App top bar + page body + optional footer.
#[component]
pub fn ProductChrome(
    #[prop(default = true)] footer: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="app-frame">
            <AppTopBar/>
            <div class="app-frame-body">{children()}</div>
            <Show when=move || footer>
                <ProductChromeFooter/>
            </Show>
        </div>
    }
}

pub mod public_layout;
