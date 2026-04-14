#[component]
pub fn DegradationPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (status, set_status) = signal(Option::<DegradationStatusResponse>::None);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());

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
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let current_locale = locale.get_untracked();
            spawn(async move {
                match client.get_degradation_status().await {
                    Ok(resp) => set_status.set(Some(resp)),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载降级状态失败",
                            "Failed to load degradation status"
                        ),
                        error
                    )),
                }
                set_loading.set(false);
            });
        },
    );

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-bold text-foreground">{move || choose(locale.get(), "降级状态", "Degradation")}</h1>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <Show when=move || loading.get()>
                <span class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "正在加载降级状态...", "Loading degradation status...")}
                </span>
            </Show>
            <Show when=move || !loading.get() && status.get().is_none() && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "暂时没有可显示的降级状态。", "No degradation status is available yet.")}
                    </div>
                </div>
            </Show>
            <Show when=move || status.get().is_some()>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "失败文档", "Failed Documents").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.failed_documents.to_string()).unwrap_or_default())
                        tone="danger"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "近 24 小时 Guard 事件", "Guard Events (24h)").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.recent_guard_events.to_string()).unwrap_or_default())
                        tone="warning"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "近 24 小时分享访问事件", "Share Access Events (24h)").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.share_access_events.to_string()).unwrap_or_default())
                        tone="primary"
                    />
                </div>
            </Show>
        </div>
    }
}

