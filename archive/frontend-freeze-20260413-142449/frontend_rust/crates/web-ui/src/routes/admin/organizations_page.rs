#[component]
pub fn OrganizationsPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    let (orgs, set_orgs) = signal(Vec::<web_sdk::dtos::OrgRow>::new());
    let (org_query, set_org_query) = signal(String::new());
    let (org_status_filter, set_org_status_filter) = signal("all".to_string());
    let (org_sort, set_org_sort) = signal("queries_desc".to_string());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());

    let filtered_orgs = move || {
        let query = org_query.get().trim().to_lowercase();
        let status_filter = org_status_filter.get();
        let filtered = orgs
            .get()
            .into_iter()
            .filter(|org| {
                if !query.is_empty()
                    && !org.name.to_lowercase().contains(&query)
                    && !org.id.to_lowercase().contains(&query)
                    && !org.plan.to_lowercase().contains(&query)
                {
                    return false;
                }

                match status_filter.as_str() {
                    "active" => !org.blocked,
                    "blocked" => org.blocked,
                    _ => true,
                }
            })
            .collect::<Vec<_>>();
        let sort = match org_sort.get().as_str() {
            "name_asc" => OrgSort::NameAsc,
            "users_desc" => OrgSort::UsersDesc,
            "notebooks_desc" => OrgSort::NotebooksDesc,
            "created_desc" => OrgSort::CreatedDesc,
            _ => OrgSort::QueriesDesc,
        };
        sort_org_rows(&filtered, sort)
    };
    let blocked_org_count = move || orgs.get().iter().filter(|org| org.blocked).count();
    let active_org_count = move || orgs.get().len().saturating_sub(blocked_org_count());
    let total_user_count = move || orgs.get().iter().map(|org| org.user_count).sum::<i64>();
    let total_notebook_count = move || orgs.get().iter().map(|org| org.notebook_count).sum::<i64>();

    // Fetch orgs on mount
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
            let set_orgs_clone = set_orgs.clone();
            let set_error_clone = set_error.clone();
            let current_locale = locale.get_untracked();

            spawn(async move {
                match client.list_orgs().await {
                    Ok(resp) => {
                        set_orgs_clone.set(resp.orgs);
                    }
                    Err(e) => {
                        set_error_clone.set(format!(
                            "{}: {}",
                            choose(
                                current_locale,
                                "加载组织列表失败",
                                "Failed to load organizations"
                            ),
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
                    {move || choose(locale.get(), "组织列表", "Organizations")}
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
                        {move || choose(locale.get(), "正在加载组织列表...", "Loading organizations...")}
                    </span>
                </div>
            </Show>

            {/* Empty state */}
            <Show when=move || !loading.get() && orgs.get().is_empty() && error.get().is_empty()>
                <div class="bg-card rounded-lg border border-border p-8 text-center">
                    <p class="text-muted-foreground">{move || choose(locale.get(), "未找到任何组织", "No organizations found")}</p>
                </div>
            </Show>

            {/* Organizations table */}
            <Show when=move || !loading.get() && !orgs.get().is_empty()>
                <div class="space-y-4">
                    <div class="grid grid-cols-2 gap-4 xl:grid-cols-4">
                        <AdminMetricCard
                            label=Signal::derive(move || choose(locale.get(), "组织总数", "Total Organizations").to_string())
                            value=Signal::derive(move || orgs.get().len().to_string())
                            tone="primary"
                        />
                        <AdminMetricCard
                            label=Signal::derive(move || choose(locale.get(), "正常组织", "Active Organizations").to_string())
                            value=Signal::derive(move || active_org_count().to_string())
                            tone="success"
                        />
                        <AdminMetricCard
                            label=Signal::derive(move || choose(locale.get(), "已封禁组织", "Blocked Organizations").to_string())
                            value=Signal::derive(move || blocked_org_count().to_string())
                            tone="danger"
                        />
                        <AdminMetricCard
                            label=Signal::derive(move || choose(locale.get(), "知识库总数", "Total Notebooks").to_string())
                            value=Signal::derive(move || total_notebook_count().to_string())
                            tone="warning"
                        />
                    </div>

                    <div class="rounded-lg border border-border bg-card px-4 py-4">
                        <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_220px_220px]">
                            <div>
                                <label class="mb-2 block text-sm font-medium text-foreground">
                                    {move || choose(locale.get(), "搜索组织", "Search organizations")}
                                </label>
                                <input
                                    type="text"
                                    class="w-full rounded border border-border px-3 py-2 text-sm"
                                    placeholder={move || choose(locale.get(), "按名称、ID 或方案筛选", "Filter by name, ID, or plan")}
                                    value=move || org_query.get()
                                    on:input=move |ev| set_org_query.set(event_target_value(&ev))
                                />
                            </div>
                            <div>
                                <label class="mb-2 block text-sm font-medium text-foreground">
                                    {move || choose(locale.get(), "组织状态", "Organization status")}
                                </label>
                                <select
                                    class="w-full rounded border border-border px-3 py-2 text-sm"
                                    on:change=move |ev| set_org_status_filter.set(event_target_value(&ev))
                                >
                                    <option value="all" selected=move || org_status_filter.get() == "all">
                                        {move || choose(locale.get(), "全部状态", "All statuses")}
                                    </option>
                                    <option value="active" selected=move || org_status_filter.get() == "active">
                                        {move || choose(locale.get(), "正常", "Active")}
                                    </option>
                                    <option value="blocked" selected=move || org_status_filter.get() == "blocked">
                                        {move || choose(locale.get(), "已封禁", "Blocked")}
                                    </option>
                                </select>
                            </div>
                            <div>
                                <label class="mb-2 block text-sm font-medium text-foreground">
                                    {move || choose(locale.get(), "排序", "Sort by")}
                                </label>
                                <select
                                    class="w-full rounded border border-border px-3 py-2 text-sm"
                                    on:change=move |ev| set_org_sort.set(event_target_value(&ev))
                                >
                                    <option value="queries_desc" selected=move || org_sort.get() == "queries_desc">
                                        {move || choose(locale.get(), "查询数优先", "Queries desc")}
                                    </option>
                                    <option value="users_desc" selected=move || org_sort.get() == "users_desc">
                                        {move || choose(locale.get(), "用户数优先", "Users desc")}
                                    </option>
                                    <option value="notebooks_desc" selected=move || org_sort.get() == "notebooks_desc">
                                        {move || choose(locale.get(), "知识库数优先", "Notebooks desc")}
                                    </option>
                                    <option value="created_desc" selected=move || org_sort.get() == "created_desc">
                                        {move || choose(locale.get(), "最近创建优先", "Newest first")}
                                    </option>
                                    <option value="name_asc" selected=move || org_sort.get() == "name_asc">
                                        {move || choose(locale.get(), "名称 A-Z", "Name A-Z")}
                                    </option>
                                </select>
                            </div>
                        </div>
                        <div class="mt-3 flex flex-wrap gap-4 text-xs text-muted-foreground">
                            <span>
                                {move || {
                                    format!(
                                        "{} {}/{}",
                                        choose(locale.get(), "匹配组织", "Matching organizations"),
                                        filtered_orgs().len(),
                                        orgs.get().len()
                                    )
                                }}
                            </span>
                            <span>
                                {move || {
                                    format!(
                                        "{} {}",
                                        choose(locale.get(), "覆盖用户", "Users covered"),
                                        filtered_orgs().iter().map(|org| org.user_count).sum::<i64>()
                                    )
                                }}
                            </span>
                            <span>
                                {move || {
                                    format!(
                                        "{} {}",
                                        choose(locale.get(), "全局用户总数", "Total users"),
                                        total_user_count()
                                    )
                                }}
                            </span>
                            <span>
                                {move || {
                                    format!(
                                        "{} {}",
                                        choose(locale.get(), "排序方式", "Sort"),
                                        match org_sort.get().as_str() {
                                            "users_desc" => choose(locale.get(), "用户数优先", "Users desc"),
                                            "notebooks_desc" => choose(locale.get(), "知识库数优先", "Notebooks desc"),
                                            "created_desc" => choose(locale.get(), "最近创建优先", "Newest first"),
                                            "name_asc" => choose(locale.get(), "名称 A-Z", "Name A-Z"),
                                            _ => choose(locale.get(), "查询数优先", "Queries desc"),
                                        }
                                    )
                                }}
                            </span>
                        </div>
                    </div>

                    <Show when=move || filtered_orgs().is_empty()>
                        <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center">
                            <div class="text-sm text-muted-foreground">
                                {move || choose(locale.get(), "没有匹配当前筛选条件的组织。", "No organizations match the current filters.")}
                            </div>
                        </div>
                    </Show>

                    <Show when=move || !filtered_orgs().is_empty()>
                        <div class="bg-card rounded-lg border border-border overflow-hidden">
                            <OrgListTable orgs={Signal::derive(filtered_orgs)} set_orgs={set_orgs} />
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// OrgDetailPage - single org detail
// ----------------------------------------------------------------------------

