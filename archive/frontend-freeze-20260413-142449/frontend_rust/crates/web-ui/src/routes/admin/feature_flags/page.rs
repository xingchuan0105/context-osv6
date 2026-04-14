#[component]
pub fn FeatureFlagsPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (flags, set_flags) = signal(Vec::<FeatureFlagEntry>::new());
    let (requests, set_requests) = signal(Vec::<FeatureFlagChangeRequest>::new());
    let (flag_query, set_flag_query) = signal(String::new());
    let (request_status_filter, set_request_status_filter) = signal("all".to_string());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());
    let (reload_nonce, set_reload_nonce) = signal(0_u64);
    let (busy_action, set_busy_action) = signal(String::new());
    let auth_for_flag_cards = auth.clone();
    let auth_for_request_cards = auth.clone();

    let total_flag_count = move || flags.get().len();
    let pending_flag_count = move || {
        flags
            .get()
            .iter()
            .filter(|flag| flag.has_pending_request)
            .count()
    };
    let config_blocked_count = move || {
        flags
            .get()
            .iter()
            .filter(|flag| flag.requires_config && !flag.config_ready)
            .count()
    };
    let drift_flag_count = move || {
        flags
            .get()
            .iter()
            .filter(|flag| flag.enabled != flag.effective_enabled)
            .count()
    };
    let filtered_flags = move || {
        let query = flag_query.get().trim().to_lowercase();
        flags
            .get()
            .into_iter()
            .filter(|flag| {
                if query.is_empty() {
                    return true;
                }

                flag.key.to_lowercase().contains(&query)
                    || flag.description.to_lowercase().contains(&query)
                    || flag.category.to_lowercase().contains(&query)
                    || flag.source.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>()
    };

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| {
                    format!(
                        "{}:{}:{}",
                        value,
                        reload_nonce.get(),
                        request_status_filter.get()
                    )
                })
                .unwrap_or_default()
        },
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            set_loading.set(true);
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let client_requests = client.clone();
            let current_locale = locale.get_untracked();
            let request_status_value = request_status_filter.get();
            spawn(async move {
                match client.list_feature_flags().await {
                    Ok(items) => set_flags.set(items),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载功能开关失败",
                            "Failed to load feature flags"
                        ),
                        error
                    )),
                }
                match client_requests
                    .list_feature_flag_change_requests(
                        (request_status_value != "all").then_some(request_status_value.as_str()),
                    )
                    .await
                {
                    Ok(items) => set_requests.set(items),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载变更申请失败",
                            "Failed to load feature flag requests"
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
            <h1 class="text-2xl font-bold text-foreground">{move || choose(locale.get(), "功能开关", "Feature Flags")}</h1>
            <p class="text-sm text-muted-foreground">
                {move || {
                    choose(
                        locale.get(),
                        "功能开关当前持久化在 PostgreSQL 中。常规变更应通过申请和审核流程完成，紧急覆盖仅用于应急场景。",
                        "Flags are now persisted in PostgreSQL. UI changes should go through request/review flow; direct override is reserved for emergencies.",
                    )
                }}
            </p>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <Show when=move || loading.get()>
                <span class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "正在加载功能开关...", "Loading feature flags...")}
                </span>
            </Show>
            <Show when=move || !loading.get() && flags.get().is_empty() && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "还没有可配置的功能开关。", "No configurable feature flags yet.")}
                    </div>
                </div>
            </Show>
            <Show when=move || !flags.get().is_empty()>
                <div class="grid grid-cols-2 gap-4 xl:grid-cols-4">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "总开关数", "Total Flags").to_string())
                        value=Signal::derive(move || total_flag_count().to_string())
                        tone="primary"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "待处理申请", "Pending Requests").to_string())
                        value=Signal::derive(move || pending_flag_count().to_string())
                        tone="warning"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "配置阻塞", "Config Blockers").to_string())
                        value=Signal::derive(move || config_blocked_count().to_string())
                        tone="danger"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "期望/生效漂移", "Desired vs Effective Drift").to_string())
                        value=Signal::derive(move || drift_flag_count().to_string())
                        tone="success"
                    />
                </div>
            </Show>
            <Show when=move || !flags.get().is_empty()>
                <div class="rounded-lg border border-border bg-card px-4 py-4">
                    <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_220px]">
                        <div>
                            <label class="mb-2 block text-sm font-medium text-foreground">
                                {move || choose(locale.get(), "搜索开关", "Search flags")}
                            </label>
                            <input
                                type="text"
                                class="w-full rounded border border-border px-3 py-2 text-sm"
                                placeholder={move || choose(locale.get(), "按 key、描述、分类或来源筛选", "Filter by key, description, category, or source")}
                                value=move || flag_query.get()
                                on:input=move |ev| set_flag_query.set(event_target_value(&ev))
                            />
                        </div>
                        <div>
                            <label class="mb-2 block text-sm font-medium text-foreground">
                                {move || choose(locale.get(), "申请状态", "Request status")}
                            </label>
                            <select
                                class="w-full rounded border border-border px-3 py-2 text-sm"
                                on:change=move |ev| set_request_status_filter.set(event_target_value(&ev))
                            >
                                <option value="all" selected=move || request_status_filter.get() == "all">
                                    {move || choose(locale.get(), "全部状态", "All statuses")}
                                </option>
                                <option value="pending" selected=move || request_status_filter.get() == "pending">
                                    {move || choose(locale.get(), "待处理", "Pending")}
                                </option>
                                <option value="approved" selected=move || request_status_filter.get() == "approved">
                                    {move || choose(locale.get(), "已批准", "Approved")}
                                </option>
                                <option value="rejected" selected=move || request_status_filter.get() == "rejected">
                                    {move || choose(locale.get(), "已拒绝", "Rejected")}
                                </option>
                                <option value="executed" selected=move || request_status_filter.get() == "executed">
                                    {move || choose(locale.get(), "已执行", "Executed")}
                                </option>
                            </select>
                        </div>
                    </div>
                    <div class="mt-3 flex flex-wrap gap-4 text-xs text-muted-foreground">
                        <span>
                            {move || {
                                format!(
                                    "{} {}/{}",
                                    choose(locale.get(), "匹配开关", "Matching flags"),
                                    filtered_flags().len(),
                                    flags.get().len()
                                )
                            }}
                        </span>
                        <span>
                            {move || {
                                format!(
                                    "{} {}",
                                    choose(locale.get(), "申请筛选", "Request filter"),
                                    if request_status_filter.get() == "all" {
                                        choose(locale.get(), "全部状态", "All statuses").to_string()
                                    } else {
                                        feature_flag_status_label(locale.get(), &request_status_filter.get())
                                    }
                                )
                            }}
                        </span>
                    </div>
                </div>
            </Show>
            <Show when=move || !flags.get().is_empty() && filtered_flags().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "没有匹配当前搜索条件的功能开关。", "No feature flags match the current search.")}
                    </div>
                </div>
            </Show>
                <div class="space-y-3">
                    {move || {
                        let auth = auth_for_flag_cards.clone();
                        filtered_flags()
                            .into_iter()
                            .map(|flag| {
                            view! {
                                <FeatureFlagCard
                                    flag=flag
                                    locale=locale
                                    auth=auth.clone()
                                    busy_action=busy_action
                                    set_busy_action=set_busy_action
                                    set_error=set_error
                                    set_reload_nonce=set_reload_nonce
                                />
                            }
                        })
                        .collect_view()
                }}
            </div>

            <div class="space-y-4">
                <h2 class="text-lg font-semibold text-foreground">{move || choose(locale.get(), "变更申请", "Change Requests")}</h2>
                <Show when=move || requests.get().is_empty() && !loading.get()>
                    <div class="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-muted-foreground">
                        <div class="text-sm text-muted-foreground">
                            {move || {
                                if request_status_filter.get() == "all" {
                                    choose(locale.get(), "还没有功能开关变更申请。", "No feature flag change requests yet.")
                                } else {
                                    choose(locale.get(), "当前筛选条件下没有变更申请。", "No change requests match the current filter.")
                                }
                            }}
                        </div>
                    </div>
                </Show>
                <div class="space-y-3">
                    {move || {
                        let auth = auth_for_request_cards.clone();
                        requests
                            .get()
                            .into_iter()
                            .map(|request| {
                                view! {
                                    <FeatureFlagRequestCard
                                        request=request
                                        locale=locale
                                        auth=auth.clone()
                                        busy_action=busy_action
                                        set_busy_action=set_busy_action
                                        set_error=set_error
                                        set_reload_nonce=set_reload_nonce
                                    />
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </div>
        </div>
    }
}
