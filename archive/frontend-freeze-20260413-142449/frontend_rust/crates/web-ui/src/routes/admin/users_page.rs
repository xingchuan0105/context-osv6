#[component]
pub fn UsersPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    let (orgs, set_orgs) = signal(Vec::<web_sdk::dtos::OrgRow>::new());
    let (users, set_users) = signal(Vec::<web_sdk::dtos::UserRow>::new());
    let (selected_org, set_selected_org) = signal(String::new());
    let (user_query, set_user_query) = signal(String::new());
    let (role_filter, set_role_filter) = signal("all".to_string());
    let (user_sort, set_user_sort) = signal("created_desc".to_string());
    let (orgs_loading, set_orgs_loading) = signal(false);
    let (users_loading, set_users_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());
    let (loaded_org_key, set_loaded_org_key) = signal(String::new());

    let filtered_users = move || {
        let query = user_query.get().trim().to_lowercase();
        let role = role_filter.get();
        let filtered = users
            .get()
            .into_iter()
            .filter(|user| {
                if role != "all" && user.role != role {
                    return false;
                }

                if query.is_empty() {
                    return true;
                }

                user.email.to_lowercase().contains(&query)
                    || user.full_name.to_lowercase().contains(&query)
                    || user.role.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        let sort = match user_sort.get().as_str() {
            "email_asc" => UserSort::EmailAsc,
            "role_asc" => UserSort::RoleAsc,
            "last_active_desc" => UserSort::LastActiveDesc,
            _ => UserSort::CreatedDesc,
        };
        sort_user_rows(&filtered, sort)
    };

    let selected_org_name = move || {
        let current = selected_org.get();
        orgs.get()
            .into_iter()
            .find(|org| org.id == current)
            .map(|org| org.name)
            .unwrap_or_default()
    };
    let owner_count = move || {
        users
            .get()
            .iter()
            .filter(|user| user.role == "owner")
            .count()
    };
    let admin_count = move || {
        users
            .get()
            .iter()
            .filter(|user| user.role == "admin")
            .count()
    };
    let member_count = move || {
        users
            .get()
            .iter()
            .filter(|user| matches!(user.role.as_str(), "member" | "viewer" | "editor"))
            .count()
    };
    let never_active_count = move || {
        users
            .get()
            .iter()
            .filter(|user| user.last_active_at.is_none())
            .count()
    };

    // Fetch users on mount
    let auth_for_orgs = auth.clone();
    run_once_after_hydration(
        move || auth_for_orgs.token.get().unwrap_or_default(),
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            set_orgs_loading.set(true);
            set_error.set(String::new());

            let client = ApiClient::new(api_base_url()).with_auth(token);
            let client_orgs = client.clone();
            let current_locale = locale.get_untracked();
            spawn(async move {
                match client_orgs.list_orgs().await {
                    Ok(resp) => set_orgs.set(resp.orgs),
                    Err(error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                current_locale,
                                "加载组织列表失败",
                                "Failed to load organizations"
                            ),
                            error
                        ));
                    }
                }
                set_orgs_loading.set(false);
            });
        },
    );

    let auth_for_users = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_users
                .token
                .get()
                .map(|value| format!("{}:{}", value, selected_org.get()))
                .unwrap_or_default()
        },
        loaded_org_key,
        set_loaded_org_key,
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            let selected_org_id = selected_org.get();
            if selected_org_id.is_empty() {
                set_users.set(Vec::new());
                return;
            }
            set_users_loading.set(true);
            set_error.set(String::new());
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let current_locale = locale.get_untracked();
            spawn(async move {
                match client.list_users_for_org(&selected_org_id).await {
                    Ok(resp) => set_users.set(resp.users),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(current_locale, "加载用户失败", "Failed to load users"),
                        error
                    )),
                }
                set_users_loading.set(false);
            });
        },
    );

    view! {
        <div class="space-y-6">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-bold text-foreground">{move || choose(locale.get(), "用户", "Users")}</h1>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {move || choose(locale.get(), "先选择组织，再按邮箱或角色筛选用户。", "Select an organization, then filter users by email or role.")}
                    </p>
                </div>
            </div>

            <div class="bg-card rounded-lg border border-border p-4">
                <div class="grid gap-4 lg:grid-cols-[minmax(0,220px)_minmax(0,1fr)_220px_220px]">
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "组织", "Organization")}
                        </label>
                        <select
                            class="w-full rounded border border-border px-3 py-2"
                            disabled=move || orgs_loading.get() || orgs.get().is_empty()
                            on:change=move |ev| {
                                set_selected_org.set(event_target_value(&ev));
                                set_users.set(Vec::new());
                                set_error.set(String::new());
                            }
                        >
                            <option value="">
                                {move || choose(locale.get(), "请选择组织", "Select an organization")}
                            </option>
                            {orgs.get().into_iter().map(|org| {
                                let org_id = org.id.clone();
                                view! {
                                    <option value={org_id.clone()} selected={move || selected_org.get() == org_id}>{org.name}</option>
                                }
                            }).collect_view()}
                        </select>
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "筛选", "Filter")}
                        </label>
                        <input
                            type="text"
                            class="w-full rounded border border-border px-3 py-2"
                            disabled=move || selected_org.get().is_empty()
                            placeholder={move || choose(locale.get(), "按邮箱、姓名或角色筛选", "Filter by email, name, or role")}
                            value=move || user_query.get()
                            on:input=move |ev| set_user_query.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "角色", "Role")}
                        </label>
                        <select
                            class="w-full rounded border border-border px-3 py-2"
                            disabled=move || selected_org.get().is_empty()
                            on:change=move |ev| set_role_filter.set(event_target_value(&ev))
                        >
                            <option value="all" selected=move || role_filter.get() == "all">
                                {move || choose(locale.get(), "全部角色", "All roles")}
                            </option>
                            <option value="owner" selected=move || role_filter.get() == "owner">
                                {move || choose(locale.get(), "所有者", "Owner")}
                            </option>
                            <option value="admin" selected=move || role_filter.get() == "admin">
                                {move || choose(locale.get(), "管理员", "Admin")}
                            </option>
                            <option value="member" selected=move || role_filter.get() == "member">
                                {move || choose(locale.get(), "成员", "Member")}
                            </option>
                            <option value="editor" selected=move || role_filter.get() == "editor">
                                {move || choose(locale.get(), "编辑者", "Editor")}
                            </option>
                            <option value="viewer" selected=move || role_filter.get() == "viewer">
                                {move || choose(locale.get(), "查看者", "Viewer")}
                            </option>
                        </select>
                    </div>
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "排序", "Sort by")}
                        </label>
                        <select
                            class="w-full rounded border border-border px-3 py-2"
                            disabled=move || selected_org.get().is_empty()
                            on:change=move |ev| set_user_sort.set(event_target_value(&ev))
                        >
                            <option value="created_desc" selected=move || user_sort.get() == "created_desc">
                                {move || choose(locale.get(), "最近创建优先", "Newest first")}
                            </option>
                            <option value="last_active_desc" selected=move || user_sort.get() == "last_active_desc">
                                {move || choose(locale.get(), "最近活跃优先", "Latest active")}
                            </option>
                            <option value="email_asc" selected=move || user_sort.get() == "email_asc">
                                {move || choose(locale.get(), "邮箱 A-Z", "Email A-Z")}
                            </option>
                            <option value="role_asc" selected=move || user_sort.get() == "role_asc">
                                {move || choose(locale.get(), "角色分组", "Role grouping")}
                            </option>
                        </select>
                    </div>
                </div>
                <div class="mt-3 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
                    <span>
                        {move || {
                            if selected_org.get().is_empty() {
                                choose(locale.get(), "当前未选择组织", "No organization selected").to_string()
                            } else {
                                format!(
                                    "{}{}",
                                    choose(locale.get(), "当前组织：", "Current organization: "),
                                    selected_org_name(),
                                )
                            }
                        }}
                    </span>
                    <Show when=move || !selected_org.get().is_empty()>
                        <span>
                            {move || {
                                format!(
                                    "{} {}",
                                    filtered_users().len(),
                                    choose(locale.get(), "位用户", "users"),
                                )
                            }}
                        </span>
                    </Show>
                    <Show when=move || !selected_org.get().is_empty()>
                        <span>
                            {move || {
                                format!(
                                    "{} {}",
                                    choose(locale.get(), "角色筛选：", "Role filter: "),
                                    if role_filter.get() == "all" {
                                        choose(locale.get(), "全部角色", "All roles").to_string()
                                    } else {
                                        admin_user_role_label(locale.get(), &role_filter.get())
                                    }
                                )
                            }}
                        </span>
                    </Show>
                    <Show when=move || !selected_org.get().is_empty()>
                        <span>
                            {move || {
                                format!(
                                    "{} {}",
                                    choose(locale.get(), "排序：", "Sort: "),
                                    match user_sort.get().as_str() {
                                        "last_active_desc" => choose(locale.get(), "最近活跃优先", "Latest active"),
                                        "email_asc" => choose(locale.get(), "邮箱 A-Z", "Email A-Z"),
                                        "role_asc" => choose(locale.get(), "角色分组", "Role grouping"),
                                        _ => choose(locale.get(), "最近创建优先", "Newest first"),
                                    }
                                )
                            }}
                        </span>
                    </Show>
                </div>
            </div>

            {/* Error message */}
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>

            <Show when=move || orgs_loading.get()>
                <div class="flex items-center justify-center py-12">
                    <span class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "正在加载组织列表...", "Loading organizations...")}
                    </span>
                </div>
            </Show>

            <Show when=move || !orgs_loading.get() && orgs.get().is_empty() && error.get().is_empty()>
                <div class="bg-card rounded-lg border border-border p-8 text-center">
                    <p class="text-muted-foreground">{move || choose(locale.get(), "未找到任何组织", "No organizations found")}</p>
                </div>
            </Show>

            <Show when=move || !orgs_loading.get() && !orgs.get().is_empty() && selected_org.get().is_empty() && error.get().is_empty()>
                <div class="rounded-lg border border-sky-200 bg-sky-50 px-4 py-4 text-sm text-sky-900">
                    {move || choose(locale.get(), "选择一个组织后再加载用户列表。", "Choose an organization to load its users.")}
                </div>
            </Show>

            <Show when=move || users_loading.get()>
                <div class="flex items-center justify-center py-12">
                    <span class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "正在加载用户...", "Loading users...")}
                    </span>
                </div>
            </Show>

            <Show when=move || !users_loading.get() && !selected_org.get().is_empty() && !users.get().is_empty()>
                <div class="grid grid-cols-2 gap-4 xl:grid-cols-4">
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "所有者/管理员", "Owners/Admins").to_string())
                        value=Signal::derive(move || format!("{}/{}", owner_count(), admin_count()))
                        tone="primary"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "成员型角色", "Member Roles").to_string())
                        value=Signal::derive(move || member_count().to_string())
                        tone="success"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "从未活跃", "Never Active").to_string())
                        value=Signal::derive(move || never_active_count().to_string())
                        tone="warning"
                    />
                    <AdminMetricCard
                        label=Signal::derive(move || choose(locale.get(), "当前筛选结果", "Filtered Results").to_string())
                        value=Signal::derive(move || filtered_users().len().to_string())
                        tone="danger"
                    />
                </div>
            </Show>

            <Show when=move || !users_loading.get() && !selected_org.get().is_empty() && filtered_users().is_empty() && error.get().is_empty()>
                <div class="bg-card rounded-lg border border-border p-8 text-center">
                    <p class="text-muted-foreground">
                        {move || {
                            if users.get().is_empty() {
                                choose(locale.get(), "该组织下暂无用户", "No users found for this organization")
                            } else {
                                choose(locale.get(), "没有匹配当前筛选条件的用户", "No users match the current filter")
                            }
                        }}
                    </p>
                </div>
            </Show>

            <Show when=move || !users_loading.get() && !selected_org.get().is_empty() && !filtered_users().is_empty()>
                <div class="bg-card rounded-lg border border-border overflow-hidden">
                    <UserListTable users={Signal::derive(filtered_users)} />
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// UsagePage - platform usage
// ----------------------------------------------------------------------------

