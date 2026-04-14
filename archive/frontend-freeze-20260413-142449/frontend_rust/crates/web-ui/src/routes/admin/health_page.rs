#[component]
pub fn HealthPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    let (health, set_health) = signal(Option::<HealthResponse>::None);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());

    // Fetch health on mount
    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || auth_for_load.token.get().unwrap_or_default(),
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            set_loading.set(true);
            set_error.set(String::new());

            let client = ApiClient::new(api_base_url()).with_auth(token);
            let set_health_clone = set_health.clone();
            let set_error_clone = set_error.clone();
            let current_locale = locale.get_untracked();

            spawn(async move {
                match client.get_health().await {
                    Ok(resp) => {
                        set_health_clone.set(Some(resp));
                    }
                    Err(e) => {
                        set_error_clone.set(format!(
                            "{}: {}",
                            choose(current_locale, "加载健康状态失败", "Failed to load health"),
                            e
                        ));
                    }
                }
                set_loading.set(false);
            });
        },
    );

    view! {
        <div class="space-y-6">
            <div class="flex items-center justify-between">
                <h1 class="text-2xl font-bold text-foreground">
                    {move || choose(locale.get(), "系统健康", "System Health")}
                </h1>
            </div>

            {/* Error message */}
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>

            {/* Loading indicator */}
            <Show when=move || loading.get()>
                <div class="flex items-center justify-center py-12">
                    <span class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "正在加载健康数据...", "Loading health data...")}
                    </span>
                </div>
            </Show>

            {/* Health status */}
            <Show when=move || !loading.get()>
                <HealthStatus health={health.get()} />
            </Show>
        </div>
    }
}
