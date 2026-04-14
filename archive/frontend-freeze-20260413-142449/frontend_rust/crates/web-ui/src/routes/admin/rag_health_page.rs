#[component]
pub fn RagHealthPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (status, set_status) = signal(Option::<RagHealthStatus>::None);
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
                match client.get_rag_health().await {
                    Ok(resp) => set_status.set(Some(resp)),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载 RAG 健康状态失败",
                            "Failed to load rag health"
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
            <h1 class="text-2xl font-bold text-foreground">{move || choose(locale.get(), "RAG 健康", "RAG Health")}</h1>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <Show when=move || loading.get()>
                <span class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "正在加载 RAG 健康状态...", "Loading RAG health...")}
                </span>
            </Show>
            <Show when=move || !loading.get() && status.get().is_none() && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "暂时没有可显示的 RAG 健康数据。", "No RAG health data is available yet.")}
                    </div>
                </div>
            </Show>
            <Show when=move || status.get().is_some()>
                <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "失败文档", "Failed Docs").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.failed_documents.to_string()).unwrap_or_default())
                        tone="danger"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "排队任务", "Queued Tasks").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.queued_tasks.to_string()).unwrap_or_default())
                        tone="warning"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "处理中", "Processing").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.processing_tasks.to_string()).unwrap_or_default())
                        tone="primary"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "Guard 事件", "Guard Events").to_string())
                        value=Signal::derive(move || status.get().as_ref().map(|s| s.recent_guard_events.to_string()).unwrap_or_default())
                        tone="success"
                    />
                </div>
            </Show>
        </div>
    }
}
