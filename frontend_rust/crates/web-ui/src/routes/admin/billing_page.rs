#[component]
pub fn BillingPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (overview, set_overview) = signal(Option::<BillingOverview>::None);
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
                match client.get_billing_overview().await {
                    Ok(resp) => set_overview.set(Some(resp)),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载账单概览失败",
                            "Failed to load billing overview"
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
            <h1 class="text-2xl font-bold text-foreground">
                {move || choose(locale.get(), "账单概览", "Billing Overview")}
            </h1>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <Show when=move || loading.get()>
                <span class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "正在加载账单概览...", "Loading billing overview...")}
                </span>
            </Show>
            <Show when=move || !loading.get() && overview.get().is_none() && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "暂时没有可显示的账单概览。", "No billing overview is available yet.")}
                    </div>
                </div>
            </Show>
            <Show when=move || overview.get().is_some()>
                <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "活跃订阅", "Active").to_string())
                        value=Signal::derive(move || overview.get().as_ref().map(|s| s.active_subscriptions.to_string()).unwrap_or_default())
                        tone="success"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "逾期未付", "Past Due").to_string())
                        value=Signal::derive(move || overview.get().as_ref().map(|s| s.past_due_subscriptions.to_string()).unwrap_or_default())
                        tone="warning"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "未支付", "Unpaid").to_string())
                        value=Signal::derive(move || overview.get().as_ref().map(|s| s.unpaid_subscriptions.to_string()).unwrap_or_default())
                        tone="danger"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "已取消", "Canceled").to_string())
                        value=Signal::derive(move || overview.get().as_ref().map(|s| s.canceled_subscriptions.to_string()).unwrap_or_default())
                        tone="primary"
                    />
                </div>
            </Show>
        </div>
    }
}
