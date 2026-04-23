#[component]
pub fn OrgDetailPage() -> impl IntoView {
    let params = use_params_map();
    let org_id = move || params.get().get("org_id").unwrap_or_default();

    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    let (org_response, set_org_response) = signal(Option::<OrgResponse>::None);
    let (org_users, set_org_users) = signal(Vec::<UserRow>::new());
    let (usage_7d, set_usage_7d) = signal(Option::<AdminUsageResponse>::None);
    let (usage_30d, set_usage_30d) = signal(Option::<AdminUsageResponse>::None);
    let (loading, set_loading) = signal(false);
    let (insight_loading, set_insight_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (insight_error, set_insight_error) = signal(String::new());
    let (loaded_key, set_loaded_key) = signal(String::new());

    // Fetch org detail on mount
    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| format!("{}:{}", value, org_id()))
                .unwrap_or_default()
        },
        loaded_key,
        set_loaded_key,
        move || {
            let org_id_val = org_id();
            let Some(token) = auth.token.get() else {
                return;
            };
            if org_id_val.is_empty() {
                return;
            }
            set_loading.set(true);
            set_error.set(String::new());
            set_insight_error.set(String::new());
            set_insight_loading.set(true);

            let client = ApiClient::new(api_base_url()).with_auth(token);
            let set_org_response_clone = set_org_response.clone();
            let set_error_clone = set_error.clone();
            let current_locale = locale.get_untracked();

            spawn(async move {
                match client.get_org(&org_id_val).await {
                    Ok(resp) => {
                        set_org_response_clone.set(Some(resp));
                        match client.list_users_for_org(&org_id_val).await {
                            Ok(resp) => set_org_users.set(resp.users),
                            Err(error) => set_insight_error.set(format!(
                                "{}: {}",
                                choose(
                                    current_locale,
                                    "加载组织成员失败",
                                    "Failed to load organization members"
                                ),
                                error
                            )),
                        }
                        match client
                            .get_admin_usage_for_org_with_period(&org_id_val, "7d")
                            .await
                        {
                            Ok(resp) => set_usage_7d.set(Some(resp)),
                            Err(error) if insight_error.get_untracked().is_empty() => {
                                set_insight_error.set(format!(
                                    "{}: {}",
                                    choose(
                                        current_locale,
                                        "加载近 7 天用量失败",
                                        "Failed to load 7d usage"
                                    ),
                                    error
                                ));
                            }
                            Err(_) => {}
                        }
                        match client
                            .get_admin_usage_for_org_with_period(&org_id_val, "30d")
                            .await
                        {
                            Ok(resp) => set_usage_30d.set(Some(resp)),
                            Err(error) if insight_error.get_untracked().is_empty() => {
                                set_insight_error.set(format!(
                                    "{}: {}",
                                    choose(
                                        current_locale,
                                        "加载近 30 天用量失败",
                                        "Failed to load 30d usage"
                                    ),
                                    error
                                ));
                            }
                            Err(_) => {}
                        }
                    }
                    Err(e) => {
                        set_error_clone.set(format!(
                            "{}: {}",
                            choose(
                                current_locale,
                                "加载组织详情失败",
                                "Failed to load organization"
                            ),
                            e
                        ));
                    }
                }
                set_loading.set(false);
                set_insight_loading.set(false);
            });
        },
    );

    let owner_count = move || {
        org_users
            .get()
            .iter()
            .filter(|user| user.role == "owner")
            .count()
    };
    let admin_count = move || {
        org_users
            .get()
            .iter()
            .filter(|user| user.role == "admin")
            .count()
    };
    let member_count = move || {
        org_users
            .get()
            .iter()
            .filter(|user| matches!(user.role.as_str(), "member" | "viewer" | "editor"))
            .count()
    };
    let recent_members = move || {
        let sorted = sort_user_rows(&org_users.get(), UserSort::CreatedDesc);
        sorted.into_iter().take(5).collect::<Vec<_>>()
    };
    let request_per_user_30d = move || {
        let users = org_response
            .get()
            .map(|resp| resp.org.user_count.max(1))
            .unwrap_or(1);
        usage_30d
            .get()
            .map(|usage| usage.total_requests / users)
            .unwrap_or_default()
    };
    let notebook_per_user = move || {
        org_response
            .get()
            .map(|resp| {
                if resp.org.user_count <= 0 {
                    0
                } else {
                    resp.org.notebook_count / resp.org.user_count.max(1)
                }
            })
            .unwrap_or_default()
    };

    view! {
        <div class="space-y-6">
            {/* Header */}
            <div class="flex items-center gap-4">
                <A href="/admin" attr:class="text-muted-foreground hover:text-foreground flex items-center gap-1">
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"/>
                    </svg>
                    {move || choose(locale.get(), "返回", "Back")}
                </A>
                <h1 class="text-2xl font-bold text-foreground">
                    {move || choose(locale.get(), "组织详情", "Organization Detail")}
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
                        {move || choose(locale.get(), "正在加载组织详情...", "Loading organization...")}
                    </span>
                </div>
            </Show>

            {/* Org detail panel */}
            <Show when=move || !loading.get() && org_response.get().is_some()>
                <div class="space-y-6">
                    <OrgDetailPanel org={org_response.get().unwrap().org} />

                    <Show when=move || !insight_error.get().is_empty()>
                        <ErrorBanner message={insight_error.get()} />
                    </Show>

                    <Show when=move || insight_loading.get()>
                        <div class="rounded-lg border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground">
                            {move || choose(locale.get(), "正在加载组织运营信息...", "Loading organization insights...")}
                        </div>
                    </Show>

                    <Show when=move || !insight_loading.get()>
                        <div class="grid grid-cols-2 gap-4 xl:grid-cols-4">
                            <AdminMetricCard
                                label=Signal::derive(move || choose(locale.get(), "近 7 天请求", "Requests (7d)").to_string())
                                value=Signal::derive(move || usage_7d.get().map(|usage| usage.total_requests.to_string()).unwrap_or_else(|| "0".to_string()))
                                tone="primary"
                            />
                            <AdminMetricCard
                                label=Signal::derive(move || choose(locale.get(), "近 30 天请求", "Requests (30d)").to_string())
                                value=Signal::derive(move || usage_30d.get().map(|usage| usage.total_requests.to_string()).unwrap_or_else(|| "0".to_string()))
                                tone="success"
                            />
                            <AdminMetricCard
                                label=Signal::derive(move || choose(locale.get(), "近 30 天令牌", "Tokens (30d)").to_string())
                                value=Signal::derive(move || usage_30d.get().map(|usage| usage.total_tokens.to_string()).unwrap_or_else(|| "0".to_string()))
                                tone="warning"
                            />
                            <AdminMetricCard
                                label=Signal::derive(move || choose(locale.get(), "近 30 天资料", "Documents (30d)").to_string())
                                value=Signal::derive(move || usage_30d.get().map(|usage| usage.total_documents.to_string()).unwrap_or_else(|| "0".to_string()))
                                tone="danger"
                            />
                        </div>

                        <div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                            <div class="rounded-xl border border-slate-200 bg-card p-6 shadow-sm">
                                <div class="flex items-center justify-between gap-3">
                                    <h3 class="text-lg font-semibold text-foreground">
                                        {move || choose(locale.get(), "团队构成", "Team Composition")}
                                    </h3>
                                    <span class="text-xs text-muted-foreground">
                                        {move || format!("{} {}", org_users.get().len(), choose(locale.get(), "位成员", "members"))}
                                    </span>
                                </div>
                                <div class="mt-4 grid grid-cols-3 gap-3">
                                    <div class="rounded-lg bg-slate-50 px-4 py-3">
                                        <div class="text-xs text-muted-foreground">{move || choose(locale.get(), "所有者", "Owners")}</div>
                                        <div class="mt-2 text-xl font-semibold text-foreground">{move || owner_count().to_string()}</div>
                                    </div>
                                    <div class="rounded-lg bg-slate-50 px-4 py-3">
                                        <div class="text-xs text-muted-foreground">{move || choose(locale.get(), "管理员", "Admins")}</div>
                                        <div class="mt-2 text-xl font-semibold text-foreground">{move || admin_count().to_string()}</div>
                                    </div>
                                    <div class="rounded-lg bg-slate-50 px-4 py-3">
                                        <div class="text-xs text-muted-foreground">{move || choose(locale.get(), "成员型角色", "Member roles")}</div>
                                        <div class="mt-2 text-xl font-semibold text-foreground">{move || member_count().to_string()}</div>
                                    </div>
                                </div>

                                <div class="mt-5">
                                    <div class="mb-3 text-sm font-medium text-foreground">
                                        {move || choose(locale.get(), "最近加入成员", "Recent Members")}
                                    </div>
                                    <Show when=move || !recent_members().is_empty() fallback=move || view! {
                                        <div class="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-muted-foreground">
                                            {move || choose(locale.get(), "暂时没有成员明细。", "No member details yet.")}
                                        </div>
                                    }>
                                        <div class="space-y-2">
                                            {recent_members().into_iter().map(|user| {
                                                view! {
                                                    <div class="flex items-center justify-between rounded-lg border border-border px-3 py-3">
                                                        <div class="min-w-0">
                                                            <div class="truncate text-sm font-medium text-foreground">{user.email.clone()}</div>
                                                            <div class="mt-1 text-xs text-muted-foreground">
                                                                {admin_user_role_label(locale.get(), &user.role)}
                                                            </div>
                                                        </div>
                                                        <div class="text-xs text-muted-foreground">
                                                            {format_unix_timestamp(parse_timestamp_like(&user.created_at))}
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </Show>
                                </div>
                            </div>

                            <div class="rounded-xl border border-slate-200 bg-card p-6 shadow-sm">
                                <h3 class="text-lg font-semibold text-foreground">
                                    {move || choose(locale.get(), "运营信号", "Operational Signals")}
                                </h3>
                                <dl class="mt-4 space-y-3">
                                    <div class="flex items-center justify-between gap-4">
                                        <dt class="text-sm text-muted-foreground">{move || choose(locale.get(), "近 30 天人均请求", "Requests per user (30d)")}</dt>
                                        <dd class="text-sm font-medium text-foreground">{move || request_per_user_30d().to_string()}</dd>
                                    </div>
                                    <div class="flex items-center justify-between gap-4">
                                        <dt class="text-sm text-muted-foreground">{move || choose(locale.get(), "人均知识库", "Notebooks per user")}</dt>
                                        <dd class="text-sm font-medium text-foreground">{move || notebook_per_user().to_string()}</dd>
                                    </div>
                                    <div class="flex items-center justify-between gap-4">
                                        <dt class="text-sm text-muted-foreground">{move || choose(locale.get(), "封禁状态", "Block status")}</dt>
                                        <dd class="text-sm font-medium text-foreground">
                                            {move || {
                                                org_response
                                                    .get()
                                                    .map(|resp| {
                                                        if resp.org.blocked {
                                                            choose(locale.get(), "已封禁", "Blocked")
                                                        } else {
                                                            choose(locale.get(), "正常", "Active")
                                                        }
                                                    })
                                                    .unwrap_or_else(|| choose(locale.get(), "未知", "Unknown"))
                                            }}
                                        </dd>
                                    </div>
                                    <div class="flex items-center justify-between gap-4">
                                        <dt class="text-sm text-muted-foreground">{move || choose(locale.get(), "最近 7 天/30 天请求比", "7d vs 30d request ratio")}</dt>
                                        <dd class="text-sm font-medium text-foreground">
                                            {move || {
                                                let seven = usage_7d.get().map(|usage| usage.total_requests).unwrap_or_default();
                                                let thirty = usage_30d.get().map(|usage| usage.total_requests).unwrap_or_default();
                                                if thirty == 0 {
                                                    "0%".to_string()
                                                } else {
                                                    format!("{:.0}%", (seven as f64 / thirty as f64) * 100.0)
                                                }
                                            }}
                                        </dd>
                                    </div>
                                </dl>
                            </div>
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// UsersPage - list all users
// ----------------------------------------------------------------------------

