#[component]
pub fn SettingsPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let navigate = use_navigate();
    let query_params = leptos_router::hooks::use_query_map();
    let location = leptos_router::hooks::use_location();
    let is_preview_route =
        Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let is_preview_for_settings_base = is_preview_route.clone();
    let settings_base_path = Memo::new(move |_| {
        if is_preview_for_settings_base.get() {
            "/preview/live/settings".to_string()
        } else {
            "/settings".to_string()
        }
    });
    let (active_tab, set_active_tab) = signal(SettingsTab::Billing);
    let (tab_bootstrap_done, set_tab_bootstrap_done) = signal(false);

    let query_params_for_init = query_params.clone();
    Effect::new(move |_| {
        if tab_bootstrap_done.get() {
            return;
        }
        let requested_tab = query_params_for_init
            .get()
            .get("tab")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let initial_tab = match requested_tab.as_str() {
            "profile" => SettingsTab::Profile,
            "appearance" => SettingsTab::Appearance,
            "security" => SettingsTab::Security,
            "notifications" => SettingsTab::Notifications,
            _ => SettingsTab::Billing,
        };
        set_active_tab.set(initial_tab);
        set_tab_bootstrap_done.set(true);
    });

    let navigate_for_tabs = navigate.clone();
    let query_params_for_sync = query_params.clone();
    let settings_base_path_for_sync = settings_base_path.clone();
    Effect::new(move |_| {
        if !tab_bootstrap_done.get() {
            return;
        }
        let target_tab = match active_tab.get() {
            SettingsTab::Billing => "billing",
            SettingsTab::Profile => "profile",
            SettingsTab::Appearance => "appearance",
            SettingsTab::Security => "security",
            SettingsTab::Notifications => "notifications",
        };
        let current_tab = query_params_for_sync
            .get()
            .get("tab")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if current_tab != target_tab {
            navigate_for_tabs(
                &format!("{}?tab={target_tab}", settings_base_path_for_sync.get()),
                NavigateOptions::default(),
            );
        }
    });

    let auth_for_logout = auth.clone();
    let navigate_for_logout = navigate.clone();
    let is_preview_route_for_logout = is_preview_route.clone();
    let handle_logout = move |_| {
        auth_for_logout.logout();
        let logout_path = if is_preview_route_for_logout.get_untracked() {
            "/preview/live/login"
        } else {
            "/login"
        };
        navigate_for_logout(logout_path, NavigateOptions::default());
    };

    view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-5xl space-y-6">
                <div class="flex flex-wrap items-start justify-between gap-3 sm:gap-4">
                    <div class="app-page-heading mb-0">
                        <h1 class="app-page-title">
                            {move || choose(locale.get(), "设置", "Settings")}
                        </h1>
                        <p class="app-page-subtitle">
                            {move || choose(locale.get(), "管理账户资料、主题语言、安全和通知偏好。", "Manage your account profile, appearance, security, and notification preferences.")}
                        </p>
                    </div>
                    <button
                        class="app-button-secondary min-w-[7.5rem] justify-center whitespace-nowrap"
                        on:click=handle_logout
                    >
                        {move || choose(locale.get(), "退出登录", "Sign Out")}
                    </button>
                </div>

                <nav class="app-tab-bar">
                    <button class="app-tab-button" class=("app-tab-button-active", move || active_tab.get() == SettingsTab::Billing) on:click=move |_| set_active_tab.set(SettingsTab::Billing)>
                        {move || choose(locale.get(), "账单", "Billing")}
                    </button>
                    <button class="app-tab-button" class=("app-tab-button-active", move || active_tab.get() == SettingsTab::Profile) on:click=move |_| set_active_tab.set(SettingsTab::Profile)>
                        {move || choose(locale.get(), "资料", "Profile")}
                    </button>
                    <button class="app-tab-button" class=("app-tab-button-active", move || active_tab.get() == SettingsTab::Appearance) on:click=move |_| set_active_tab.set(SettingsTab::Appearance)>
                        {move || choose(locale.get(), "外观与语言", "Appearance")}
                    </button>
                    <button class="app-tab-button" class=("app-tab-button-active", move || active_tab.get() == SettingsTab::Security) on:click=move |_| set_active_tab.set(SettingsTab::Security)>
                        {move || choose(locale.get(), "安全", "Security")}
                    </button>
                    <button class="app-tab-button" class=("app-tab-button-active", move || active_tab.get() == SettingsTab::Notifications) on:click=move |_| set_active_tab.set(SettingsTab::Notifications)>
                        {move || choose(locale.get(), "通知", "Notifications")}
                    </button>
                </nav>

                <div class:hidden=move || active_tab.get() != SettingsTab::Billing>
                    <BillingPanel />
                </div>
                <div class:hidden=move || active_tab.get() != SettingsTab::Profile>
                    <ProfileSettings />
                </div>
                <div class:hidden=move || active_tab.get() != SettingsTab::Appearance>
                    <AppearanceSettings />
                </div>
                <div class:hidden=move || active_tab.get() != SettingsTab::Security>
                    <SecuritySettings />
                </div>
                <div class:hidden=move || active_tab.get() != SettingsTab::Notifications>
                    <NotificationSettings />
                </div>
            </div>
        </div>
    }
}
