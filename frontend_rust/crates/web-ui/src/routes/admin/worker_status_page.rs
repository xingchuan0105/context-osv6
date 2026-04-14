#[component]
pub fn WorkerStatusPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (status, set_status) = signal(Option::<WorkerStatusResponse>::None);
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
                match client.get_worker_status().await {
                    Ok(resp) => set_status.set(Some(resp)),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载执行器状态失败",
                            "Failed to load worker status"
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
            <h1 class="text-2xl font-bold text-foreground">{move || choose(locale.get(), "执行器状态", "Worker Status")}</h1>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <Show when=move || loading.get()>
                <span class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "正在加载执行器状态...", "Loading worker status...")}
                </span>
            </Show>
            <Show when=move || !loading.get() && status.get().is_none() && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "暂时没有可显示的执行器状态。", "No worker status is available yet.")}
                    </div>
                </div>
            </Show>
            <Show when=move || status.get().is_some()>
                <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "运行模式", "Runtime").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| worker_runtime_label(locale.get(), &s.runtime_mode)).unwrap_or_default())
                        tone="primary"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "排队任务", "Queued").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.queued_tasks.to_string()).unwrap_or_default())
                        tone="warning"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "处理中", "Processing").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.processing_tasks.to_string()).unwrap_or_default())
                        tone="success"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "失败文档", "Failed Docs").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.failed_documents.to_string()).unwrap_or_default())
                        tone="danger"
                    />
                </div>
            </Show>
        </div>
    }
}

