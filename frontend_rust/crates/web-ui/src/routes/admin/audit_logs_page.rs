#[component]
pub fn AuditLogsPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (logs, set_logs) = signal(Vec::<AuditLogEntry>::new());
    let (total, set_total) = signal(0_usize);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (selected_limit, set_selected_limit) = signal(25_usize);
    let (query, set_query) = signal(String::new());
    let (action_filter, set_action_filter) = signal("all".to_string());
    let (resource_filter, set_resource_filter) = signal("all".to_string());
    let (actor_filter, set_actor_filter) = signal(String::new());
    let (window_filter, set_window_filter) = signal("all".to_string());
    let (page, set_page) = signal(1_usize);
    let (loaded_token, set_loaded_token) = signal(String::new());

    Effect::new(move |_| {
        query.get();
        action_filter.get();
        resource_filter.get();
        actor_filter.get();
        window_filter.get();
        selected_limit.get();
        set_page.set(1);
    });

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| {
                    format!(
                        "{}:{}:{}:{}:{}:{}:{}:{}",
                        value,
                        page.get(),
                        selected_limit.get(),
                        query.get(),
                        action_filter.get(),
                        resource_filter.get(),
                        actor_filter.get(),
                        window_filter.get(),
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
            set_error.set(String::new());
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let current_locale = locale.get_untracked();
            let request = web_sdk::dtos::AuditLogQuery {
                query: (!query.get().trim().is_empty()).then(|| query.get()),
                action: (action_filter.get() != "all").then(|| action_filter.get()),
                resource_type: (resource_filter.get() != "all").then(|| resource_filter.get()),
                actor: (!actor_filter.get().trim().is_empty()).then(|| actor_filter.get()),
                window: (window_filter.get() != "all").then(|| window_filter.get()),
                page: Some(page.get()),
                per_page: Some(selected_limit.get()),
            };
            spawn(async move {
                match client.list_audit_logs(&request).await {
                    Ok(response) => {
                        set_logs.set(response.items);
                        set_total.set(response.total);
                    }
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            current_locale,
                            "加载审计日志失败",
                            "Failed to load audit logs"
                        ),
                        error
                    )),
                }
                set_loading.set(false);
            });
        },
    );

    let total_pages = move || {
        ((total.get() + selected_limit.get().saturating_sub(1)) / selected_limit.get()).max(1)
    };

    view! {
        <div class="space-y-6">
            <h1 class="text-2xl font-bold text-foreground">{move || choose(locale.get(), "审计日志", "Audit Logs")}</h1>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <div class="rounded-lg border border-border bg-card px-4 py-4">
                <div class="grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_200px_200px_minmax(0,220px)_180px_140px]">
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "全文筛选", "Search")}
                        </label>
                        <input
                            type="text"
                            class="w-full rounded border border-border px-3 py-2 text-sm"
                            placeholder={move || choose(locale.get(), "按动作、资源 ID 或执行者筛选", "Filter by action, resource ID, or actor")}
                            value=move || query.get()
                            on:input=move |ev| set_query.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "动作", "Action")}
                        </label>
                        <input
                            type="text"
                            class="w-full rounded border border-border px-3 py-2 text-sm"
                            placeholder={move || choose(locale.get(), "例如 task_failed", "For example: task_failed")}
                            value=move || if action_filter.get() == "all" { String::new() } else { action_filter.get() }
                            on:input=move |ev| {
                                let value = event_target_value(&ev);
                                set_action_filter.set(if value.trim().is_empty() { "all".to_string() } else { value });
                            }
                        />
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "资源类型", "Resource")}
                        </label>
                        <input
                            type="text"
                            class="w-full rounded border border-border px-3 py-2 text-sm"
                            placeholder={move || choose(locale.get(), "例如 document", "For example: document")}
                            value=move || if resource_filter.get() == "all" { String::new() } else { resource_filter.get() }
                            on:input=move |ev| {
                                let value = event_target_value(&ev);
                                set_resource_filter.set(if value.trim().is_empty() { "all".to_string() } else { value });
                            }
                        />
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "执行者", "Actor")}
                        </label>
                        <input
                            type="text"
                            class="w-full rounded border border-border px-3 py-2 text-sm"
                            placeholder={move || choose(locale.get(), "按执行者 ID 筛选", "Filter by actor ID")}
                            value=move || actor_filter.get()
                            on:input=move |ev| set_actor_filter.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "时间窗口", "Time window")}
                        </label>
                        <select
                            class="w-full rounded border border-border px-3 py-2 text-sm"
                            on:change=move |ev| set_window_filter.set(event_target_value(&ev))
                        >
                            <option value="all" selected=move || window_filter.get() == "all">
                                {move || choose(locale.get(), "全部时间", "All time")}
                            </option>
                            <option value="24h" selected=move || window_filter.get() == "24h">
                                {move || choose(locale.get(), "近 24 小时", "Last 24h")}
                            </option>
                            <option value="7d" selected=move || window_filter.get() == "7d">
                                {move || choose(locale.get(), "近 7 天", "Last 7 days")}
                            </option>
                            <option value="30d" selected=move || window_filter.get() == "30d">
                                {move || choose(locale.get(), "近 30 天", "Last 30 days")}
                            </option>
                            <option value="90d" selected=move || window_filter.get() == "90d">
                                {move || choose(locale.get(), "近 90 天", "Last 90 days")}
                            </option>
                        </select>
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "每页条数", "Per page")}
                        </label>
                        <select
                            class="w-full rounded border border-border px-3 py-2 text-sm"
                            on:change=move |ev| set_selected_limit.set(event_target_value(&ev).parse::<usize>().unwrap_or(25))
                        >
                            <option value="25" selected=move || selected_limit.get() == 25>{"25"}</option>
                            <option value="50" selected=move || selected_limit.get() == 50>{"50"}</option>
                            <option value="100" selected=move || selected_limit.get() == 100>{"100"}</option>
                        </select>
                    </div>
                </div>
                <div class="mt-3 flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
                    <div class="flex flex-wrap gap-4">
                        <span>
                            {move || format!(
                                "{} {}",
                                choose(locale.get(), "匹配日志", "Matching logs"),
                                total.get()
                            )}
                        </span>
                        <span>
                            {move || format!(
                                "{} {}/{}",
                                choose(locale.get(), "页码", "Page"),
                                page.get().min(total_pages()).max(1),
                                total_pages()
                            )}
                        </span>
                    </div>
                    <button
                        type="button"
                        class="rounded border border-border px-3 py-1.5 text-xs font-medium text-foreground hover:bg-muted/40"
                        on:click=move |_| {
                            let Some(token) = auth.token.get() else {
                                return;
                            };
                            let client = ApiClient::new(api_base_url()).with_auth(token);
                            let request = web_sdk::dtos::AuditLogQuery {
                                query: (!query.get().trim().is_empty()).then(|| query.get()),
                                action: (action_filter.get() != "all").then(|| action_filter.get()),
                                resource_type: (resource_filter.get() != "all").then(|| resource_filter.get()),
                                actor: (!actor_filter.get().trim().is_empty()).then(|| actor_filter.get()),
                                window: (window_filter.get() != "all").then(|| window_filter.get()),
                                page: None,
                                per_page: None,
                            };
                            spawn(async move {
                                match client.export_audit_logs_csv(&request).await {
                                    Ok(csv) => {
                                        if let Err(export_error) = export_text_file("audit-logs.csv", &csv) {
                                            set_error.set(format!(
                                                "{}: {}",
                                                choose(locale.get_untracked(), "导出审计日志失败", "Failed to export audit logs"),
                                                export_error
                                            ));
                                        }
                                    }
                                    Err(error) => {
                                        set_error.set(format!(
                                            "{}: {}",
                                            choose(locale.get_untracked(), "导出审计日志失败", "Failed to export audit logs"),
                                            error
                                        ));
                                    }
                                }
                            });
                        }
                    >
                        {move || choose(locale.get(), "导出 CSV", "Export CSV")}
                    </button>
                </div>
            </div>
            <Show when=move || loading.get()>
                <span class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "正在加载审计日志...", "Loading audit logs...")}
                </span>
            </Show>
            <Show when=move || !loading.get() && total.get() == 0 && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "还没有审计日志。", "No audit logs yet.")}
                    </div>
                </div>
            </Show>
            <Show when=move || !loading.get() && total.get() != 0 && logs.get().is_empty() && error.get().is_empty()>
                <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                    <div class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "当前筛选条件下没有审计日志。", "No audit logs match the current filters.")}
                    </div>
                </div>
            </Show>
            <div class="rounded-lg border border-border overflow-hidden">
                <table class="min-w-full divide-y divide-gray-200">
                    <thead class="bg-muted/40">
                        <tr>
                            <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground uppercase">{move || choose(locale.get(), "动作", "Action")}</th>
                            <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground uppercase">{move || choose(locale.get(), "资源", "Resource")}</th>
                            <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground uppercase">{move || choose(locale.get(), "执行者", "Actor")}</th>
                            <th class="px-4 py-2 text-left text-xs font-medium text-muted-foreground uppercase">{move || choose(locale.get(), "创建时间", "Created")}</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-gray-200 bg-card">
                        {logs.get().into_iter().map(|entry| {
                            view! {
                                <tr>
                                    <td class="px-4 py-2 text-sm text-foreground">
                                        <span class="inline-flex items-center rounded-full bg-slate-100 px-2.5 py-1 text-xs font-medium text-slate-700">
                                            {audit_action_label(locale.get(), &entry.action)}
                                        </span>
                                    </td>
                                    <td class="px-4 py-2 text-sm text-muted-foreground">
                                        <div class="font-medium text-foreground">
                                            {audit_resource_type_label(locale.get(), &entry.resource_type)}
                                        </div>
                                        <div class="mt-1 text-xs text-muted-foreground">{entry.resource_id}</div>
                                    </td>
                                    <td class="px-4 py-2 text-sm text-muted-foreground">{entry.actor_id.unwrap_or_else(|| choose(locale.get(), "系统", "system").to_string())}</td>
                                    <td class="px-4 py-2 text-sm text-muted-foreground">{format_unix_timestamp(entry.created_at)}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
            <Show when=move || total.get() != 0>
                <div class="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3 text-sm text-muted-foreground">
                    <span>
                        {move || {
                            let current_page = page.get().min(total_pages()).max(1);
                            format!(
                                "{} {}/{} · {} {}",
                                choose(locale.get(), "第", "Page"),
                                current_page,
                                total_pages(),
                                total.get(),
                                choose(locale.get(), "条匹配记录", "matching records")
                            )
                        }}
                    </span>
                    <div class="flex items-center gap-2">
                        <button
                            type="button"
                            class="rounded border border-border px-3 py-1.5 disabled:opacity-50"
                            disabled=move || page.get() <= 1
                            on:click=move |_| set_page.update(|value| *value = value.saturating_sub(1))
                        >
                            {move || choose(locale.get(), "上一页", "Previous")}
                        </button>
                        <button
                            type="button"
                            class="rounded border border-border px-3 py-1.5 disabled:opacity-50"
                            disabled=move || page.get() >= total_pages()
                            on:click=move |_| set_page.update(|value| *value += 1)
                        >
                            {move || choose(locale.get(), "下一页", "Next")}
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// OrganizationsPage - list all organizations
// ----------------------------------------------------------------------------

