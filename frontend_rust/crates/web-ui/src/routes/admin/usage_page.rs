#[component]
pub fn UsagePage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    let (orgs, set_orgs) = signal(Vec::<web_sdk::dtos::OrgRow>::new());
    let (usage, set_usage) = signal(Option::<AdminUsageResponse>::None);
    let (selected_org, set_selected_org) = signal(String::new());
    let (selected_period, set_selected_period) = signal("30d".to_string());
    let (orgs_loading, set_orgs_loading) = signal(false);
    let (usage_loading, set_usage_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (warning, set_warning) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());
    let (loaded_org_key, set_loaded_org_key) = signal(String::new());

    let selected_scope_label = move || {
        let current = selected_org.get();
        if current == ALL_ORGS_VALUE {
            return choose(
                locale.get(),
                "全部组织（聚合）",
                "All organizations (aggregate)",
            )
            .to_string();
        }
        orgs.get()
            .into_iter()
            .find(|org| org.id == current)
            .map(|org| org.name)
            .unwrap_or_default()
    };

    // Fetch usage on mount
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
                    Ok(resp) => {
                        if !resp.orgs.is_empty() && selected_org.get_untracked().is_empty() {
                            set_selected_org.set(ALL_ORGS_VALUE.to_string());
                        }
                        set_orgs.set(resp.orgs);
                    }
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

    let auth_for_usage = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_usage
                .token
                .get()
                .map(|value| format!("{}:{}:{}", value, selected_org.get(), selected_period.get()))
                .unwrap_or_default()
        },
        loaded_org_key,
        set_loaded_org_key,
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            let selected_org_id = selected_org.get();
            let selected_period_value = selected_period.get();
            if selected_org_id.is_empty() {
                set_usage.set(None);
                return;
            }
            set_usage_loading.set(true);
            set_error.set(String::new());
            set_warning.set(String::new());
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let org_snapshot = orgs.get();
            let current_locale = locale.get_untracked();
            spawn(async move {
                match load_admin_usage_for_scope(
                    client,
                    org_snapshot.clone(),
                    &selected_org_id,
                    &selected_period_value,
                )
                .await
                {
                    Ok((resp, failed_orgs)) => {
                        if !failed_orgs.is_empty() {
                            let detail = if failed_orgs.len() <= 3 {
                                failed_orgs.join("、")
                            } else {
                                format!(
                                    "{} {} {}",
                                    failed_orgs[..3].join("、"),
                                    choose(current_locale, "等", "and"),
                                    failed_orgs.len()
                                )
                            };
                            set_warning.set(format!(
                                "{} {}",
                                choose(
                                    current_locale,
                                    "部分组织的用量统计失败，当前结果为不完整汇总：",
                                    "Some organizations failed to load usage. The current aggregate is partial:",
                                ),
                                detail
                            ));
                        }
                        set_usage.set(Some(resp));
                    }
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(current_locale, "加载使用数据失败", "Failed to load usage"),
                        error
                    )),
                }
                set_usage_loading.set(false);
            });
        },
    );

    view! {
        <div class="space-y-6">
            <div class="flex items-center justify-between">
                <div>
                    <h1 class="text-2xl font-bold text-foreground">
                        {move || choose(locale.get(), "平台用量", "Platform Usage")}
                    </h1>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {move || choose(locale.get(), "默认显示全部组织的聚合结果，并支持切换时间窗口做短期或季度对比。", "View the aggregate for all organizations and switch time windows for short-term or quarterly comparisons.")}
                    </p>
                </div>
            </div>

            <div class="bg-card rounded-lg border border-border p-4">
                <div class="grid gap-4 lg:grid-cols-[minmax(0,280px)_minmax(0,1fr)]">
                    <div>
                        <label class="mb-2 block text-sm font-medium text-foreground">
                            {move || choose(locale.get(), "查看范围", "Scope")}
                        </label>
                        <select
                            class="w-full rounded border border-border px-3 py-2"
                            disabled=move || orgs_loading.get() || orgs.get().is_empty()
                            on:change=move |ev| {
                                set_selected_org.set(event_target_value(&ev));
                                set_usage.set(None);
                                set_error.set(String::new());
                                set_warning.set(String::new());
                            }
                        >
                            <option value={ALL_ORGS_VALUE} selected=move || selected_org.get() == ALL_ORGS_VALUE>
                                {move || choose(locale.get(), "全部组织（聚合）", "All organizations (aggregate)")}
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
                            {move || choose(locale.get(), "时间窗口", "Time window")}
                        </label>
                        <div class="flex flex-wrap gap-2">
                            {USAGE_PERIOD_OPTIONS.iter().copied().map(|period| {
                                view! {
                                    <button
                                        type="button"
                                        class=move || {
                                            if selected_period.get() == period {
                                                "rounded-full border border-sky-300 bg-sky-50 px-3 py-2 text-sm font-medium text-sky-700 transition-colors"
                                            } else {
                                                "rounded-full border border-border bg-card px-3 py-2 text-sm font-medium text-foreground transition-colors hover:border-border hover:text-foreground"
                                            }
                                        }
                                        on:click=move |_| {
                                            if selected_period.get_untracked() == period {
                                                return;
                                            }
                                            set_selected_period.set(period.to_string());
                                            set_usage.set(None);
                                            set_error.set(String::new());
                                            set_warning.set(String::new());
                                        }
                                    >
                                        {usage_period_label(locale.get(), period)}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                        <p class="mt-3 text-xs text-muted-foreground">
                            {move || usage_period_hint(locale.get(), &selected_period.get())}
                        </p>
                    </div>
                </div>
                <div class="mt-4 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
                    <span>
                        {move || {
                            if selected_org.get().is_empty() {
                                choose(locale.get(), "正在准备可用组织...", "Preparing organizations...").to_string()
                            } else {
                                format!(
                                    "{}{}",
                                    choose(locale.get(), "当前视图：", "Current view: "),
                                    selected_scope_label(),
                                )
                            }
                        }}
                    </span>
                    <span>
                        {move || {
                            format!(
                                "{}{}",
                                choose(locale.get(), "时间窗口：", "Time window: "),
                                usage_period_label(locale.get(), &selected_period.get()),
                            )
                        }}
                    </span>
                    <Show when=move || selected_org.get() == ALL_ORGS_VALUE && !orgs.get().is_empty()>
                        <span>
                            {move || {
                                format!(
                                    "{} {}",
                                    orgs.get().len(),
                                    choose(locale.get(), "个组织参与聚合", "organizations in aggregate"),
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

            <Show when=move || !warning.get().is_empty()>
                <div class="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                    {warning.get()}
                </div>
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

            <Show when=move || usage_loading.get()>
                <div class="flex items-center justify-center py-12">
                    <span class="text-sm text-muted-foreground">
                        {move || choose(locale.get(), "正在加载使用数据...", "Loading usage data...")}
                    </span>
                </div>
            </Show>

            <Show when=move || !usage_loading.get() && usage.get().is_some()>
                <UsageChart usage={usage.get()} />
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuditLogEntry, OrgRow, OrgSort, UserRow, UserSort, audit_logs_to_csv, filter_audit_logs,
        paginate_items, sort_org_rows, sort_user_rows,
    };

    fn sample_org(
        id: &str,
        name: &str,
        users: i64,
        notebooks: i64,
        queries: i64,
        created_at: &str,
    ) -> OrgRow {
        OrgRow {
            id: id.to_string(),
            name: name.to_string(),
            plan: "starter".to_string(),
            user_count: users,
            notebook_count: notebooks,
            query_count: queries,
            blocked: false,
            created_at: created_at.to_string(),
        }
    }

    fn sample_user(
        email: &str,
        role: &str,
        created_at: &str,
        last_active_at: Option<&str>,
    ) -> UserRow {
        UserRow {
            id: email.to_string(),
            email: email.to_string(),
            full_name: String::new(),
            org_id: "org-1".to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            last_active_at: last_active_at.map(str::to_string),
        }
    }

    #[test]
    fn sort_org_rows_by_queries_desc() {
        let rows = vec![
            sample_org("1", "Alpha", 2, 1, 4, "100"),
            sample_org("2", "Beta", 3, 2, 10, "200"),
            sample_org("3", "Gamma", 1, 1, 7, "300"),
        ];

        let sorted = sort_org_rows(&rows, OrgSort::QueriesDesc);

        assert_eq!(sorted[0].name, "Beta");
        assert_eq!(sorted[1].name, "Gamma");
        assert_eq!(sorted[2].name, "Alpha");
    }

    #[test]
    fn sort_user_rows_by_last_active_desc() {
        let rows = vec![
            sample_user("a@example.com", "member", "100", Some("300")),
            sample_user("b@example.com", "admin", "200", None),
            sample_user("c@example.com", "owner", "300", Some("900")),
        ];

        let sorted = sort_user_rows(&rows, UserSort::LastActiveDesc);

        assert_eq!(sorted[0].email, "c@example.com");
        assert_eq!(sorted[1].email, "a@example.com");
        assert_eq!(sorted[2].email, "b@example.com");
    }

    #[test]
    fn filter_audit_logs_respects_action_resource_actor_and_query() {
        let logs = vec![
            AuditLogEntry {
                id: 1,
                actor_id: Some("user-1".to_string()),
                action: "task_completed".to_string(),
                resource_type: "document".to_string(),
                resource_id: "doc-1".to_string(),
                org_id: Some("org-1".to_string()),
                created_at: 1_000_000,
            },
            AuditLogEntry {
                id: 2,
                actor_id: Some("system-worker".to_string()),
                action: "task_failed".to_string(),
                resource_type: "document_ingestion_task".to_string(),
                resource_id: "task-2".to_string(),
                org_id: Some("org-1".to_string()),
                created_at: 1_000_100,
            },
        ];

        let filtered = filter_audit_logs(
            &logs,
            "task",
            "task_failed",
            "document_ingestion_task",
            "worker",
            "all",
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 2);
    }

    #[test]
    fn paginate_items_returns_expected_page_slice() {
        let items = vec![1, 2, 3, 4, 5];
        let page = paginate_items(&items, 2, 2);
        assert_eq!(page, vec![3, 4]);
    }

    #[test]
    fn audit_logs_to_csv_emits_header_and_rows() {
        let csv = audit_logs_to_csv(&[AuditLogEntry {
            id: 1,
            actor_id: Some("user-1".to_string()),
            action: "task_completed".to_string(),
            resource_type: "document".to_string(),
            resource_id: "doc-1".to_string(),
            org_id: Some("org-1".to_string()),
            created_at: 1_000_000,
        }]);

        assert!(csv.starts_with("action,resource_type,resource_id,actor_id,created_at"));
        assert!(csv.contains("\"task_completed\""));
        assert!(csv.contains("\"doc-1\""));
    }
}

// ----------------------------------------------------------------------------
// HealthPage - system health
// ----------------------------------------------------------------------------

