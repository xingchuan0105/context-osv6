#[component]
pub fn ShareCenterPage() -> impl IntoView {
    // Get notebook_id from route params
    let params = use_params_map();
    let notebook_id = move || params.get().get("notebook_id").unwrap_or_default();

    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let location = use_location();
    let navigate = use_navigate();
    let location_for_preview = location.clone();
    let is_preview_route =
        Memo::new(move |_| location_for_preview.pathname.get().starts_with("/preview/live"));
    let workspace_href = Memo::new(move |_| {
        let nid = notebook_id();
        if nid.is_empty() {
            if is_preview_route.get() {
                "/preview/live/dashboard".to_string()
            } else {
                "/dashboard".to_string()
            }
        } else if is_preview_route.get() {
            format!("/preview/live/workspace/{nid}")
        } else {
            format!("/dashboard/{nid}")
        }
    });
    let location_for_share_base = location.clone();
    let share_base_href = Memo::new(move |_| {
        let nid = notebook_id();
        if nid.is_empty() {
            String::new()
        } else {
            share_base_href_from_path(&location_for_share_base.pathname.get(), &nid)
        }
    });

    // Tab state
    let (active_tab, set_active_tab) = signal(share_tab_from_path(&location.pathname.get_untracked()));

    // Data state
    let (settings, set_settings) = signal(Option::<ShareSettings>::None);
    let (analytics, set_analytics) = signal(Option::<ShareAnalyticsResponse>::None);
    let (logs, set_logs) = signal(Option::<AccessLogsResponse>::None);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_settings_key, set_loaded_settings_key) = signal(String::new());
    let (members, set_members) = signal(Vec::<MemberRow>::new());
    let (invite_email, set_invite_email) = signal(String::new());
    let (invite_role, set_invite_role) = signal("viewer".to_string());
    let (inviting, set_inviting) = signal(false);
    let (pending_remove_member_id, set_pending_remove_member_id) = signal(Option::<String>::None);
    let (processing_remove_member_id, set_processing_remove_member_id) = signal(String::new());
    let (loaded_analytics_key, set_loaded_analytics_key) = signal(String::new());
    let (loaded_logs_key, set_loaded_logs_key) = signal(String::new());

    // Fetch share settings on mount
    let fetch_settings = move || {
        let nid = notebook_id();
        if nid.is_empty() {
            return;
        }
        let token = auth.token.get();
        if token.is_none() {
            return;
        }

        set_loading.set(true);
        set_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let nid_clone = nid.clone();

        spawn(async move {
            match client.get_share_settings(&nid_clone).await {
                Ok(resp) => {
                    set_settings.set(Some(resp));
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载分享设置失败",
                            "Failed to load share settings"
                        ),
                        e
                    ));
                }
            }
            set_loading.set(false);
        });
    };

    // Fetch analytics
    let fetch_analytics = move || {
        let nid = notebook_id();
        if nid.is_empty() {
            return;
        }
        let token = auth.token.get();
        if token.is_none() {
            return;
        }

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let nid_clone = nid.clone();

        spawn(async move {
            match client.get_share_analytics(&nid_clone).await {
                Ok(resp) => {
                    set_analytics.set(Some(resp));
                    set_loaded_analytics_key.set(nid_clone.clone());
                    set_error.set(String::new());
                }
                Err(error) => {
                    set_analytics.set(Some(ShareAnalyticsResponse {
                        total_views: 0,
                        total_unique_visitors: 0,
                        views_by_day: Default::default(),
                    }));
                    set_loaded_analytics_key.set(nid_clone.clone());
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载分享分析失败",
                            "Failed to load share analytics"
                        ),
                        error
                    ));
                }
            }
        });
    };

    let fetch_members = move || {
        let nid = notebook_id();
        if nid.is_empty() {
            return;
        }
        let token = auth.token.get();
        if token.is_none() {
            return;
        }

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let nid_clone = nid.clone();
        spawn(async move {
            match client.list_members(&nid_clone).await {
                Ok(resp) => {
                    set_members.set(resp.members);
                    set_error.set(String::new());
                }
                Err(error) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载成员列表失败",
                            "Failed to load members"
                        ),
                        error
                    ));
                }
            }
        });
    };

    // Fetch access logs
    let fetch_logs = move || {
        let nid = notebook_id();
        if nid.is_empty() {
            return;
        }
        let token = auth.token.get();
        if token.is_none() {
            return;
        }

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let nid_clone = nid.clone();

        spawn(async move {
            match client.get_access_logs(&nid_clone).await {
                Ok(resp) => {
                    set_logs.set(Some(resp));
                    set_loaded_logs_key.set(nid_clone.clone());
                    set_error.set(String::new());
                }
                Err(error) => {
                    set_logs.set(Some(AccessLogsResponse { logs: Vec::new() }));
                    set_loaded_logs_key.set(nid_clone.clone());
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载访问日志失败",
                            "Failed to load access logs"
                        ),
                        error
                    ));
                }
            }
        });
    };

    // Initial fetch
    let auth_for_load = auth.clone();
    let fetch_settings_on_mount = fetch_settings.clone();
    let fetch_members_on_mount = fetch_members.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| format!("{}:{}", value, notebook_id()))
                .unwrap_or_default()
        },
        loaded_settings_key,
        set_loaded_settings_key,
        move || {
            fetch_settings_on_mount();
            fetch_members_on_mount();
        },
    );

    let auth_for_tab_sync = auth.clone();
    let location_for_tab_sync = location.clone();
    Effect::new(move |_| {
        let nid = notebook_id();
        let current_tab = share_tab_from_path(&location_for_tab_sync.pathname.get());
        let has_token = auth_for_tab_sync.token.get().is_some();
        set_active_tab.set(current_tab);

        if !has_token || nid.is_empty() {
            return;
        }

        match current_tab {
            ShareTab::Analytics => {
                if loaded_analytics_key.get() != nid {
                    fetch_analytics();
                }
            }
            ShareTab::AccessLogs => {
                if loaded_logs_key.get() != nid {
                    fetch_logs();
                }
            }
            ShareTab::Settings => {}
        }
    });

    let navigate_for_tabs = navigate.clone();
    let location_for_tab_click = location.clone();
    let handle_tab_change = move |tab: ShareTab| {
        set_active_tab.set(tab);
        let target_path = share_tab_href(&share_base_href.get_untracked(), tab);
        if target_path.is_empty()
            || location_for_tab_click.pathname.get_untracked() == target_path
        {
            return;
        }
        navigate_for_tabs(&target_path, NavigateOptions::default());
    };
    let handle_tab_change = StoredValue::new(handle_tab_change);

    let settings_callback: Arc<dyn Fn(ShareSettings) + Send + Sync> =
        Arc::new(move |new_settings: ShareSettings| {
            let nid = notebook_id();
            if nid.is_empty() {
                return;
            }
            let token = auth.token.get();
            if token.is_none() {
                return;
            }

            let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
            let draft = new_settings.clone();

            spawn(async move {
                match client.update_share_settings(&nid, &draft).await {
                    Ok(resp) => set_settings.set(Some(resp)),
                    Err(e) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "更新分享设置失败",
                                "Failed to update settings"
                            ),
                            e
                        ));
                    }
                }
            });
        });

    let invite_callback: Arc<dyn Fn() + Send + Sync> = {
        let auth = auth.clone();
        let notebook_id = notebook_id.clone();
        Arc::new(move || {
            let nid = notebook_id();
            if nid.is_empty() || invite_email.get().trim().is_empty() {
                return;
            }
            let Some(token) = auth.token.get() else {
                return;
            };
            set_inviting.set(true);
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let email = invite_email.get();
            let role = invite_role.get();
            spawn(async move {
                match client.invite_member(&nid, &email, &role).await {
                    Ok(_) => {
                        set_invite_email.set(String::new());
                        fetch_members();
                    }
                    Err(error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "邀请成员失败",
                                "Failed to invite member"
                            ),
                            error
                        ));
                    }
                }
                set_inviting.set(false);
            });
        })
    };

    let auth_for_member_remove = auth.clone();
    Effect::new(move |_| {
        let Some(member_id) = pending_remove_member_id.get() else {
            return;
        };
        if member_id.is_empty() || processing_remove_member_id.get() == member_id {
            return;
        }

        set_processing_remove_member_id.set(member_id.clone());
        let auth = auth_for_member_remove.clone();
        let notebook_id = notebook_id.clone();
        spawn(async move {
            let nid = notebook_id();
            if let Some(token) = auth.token.get() {
                let client = ApiClient::new(api_base_url()).with_auth(token);
                match client.remove_member(&nid, &member_id).await {
                    Ok(_) => fetch_members(),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "移除成员失败",
                            "Failed to remove member"
                        ),
                        error
                    )),
                }
            }
            set_pending_remove_member_id.set(None);
            set_processing_remove_member_id.set(String::new());
        });
    });

    // Handle enable/disable toggle
    let toggle_callback: Arc<dyn Fn(ShareSettings) + Send + Sync> =
        Arc::new(move |draft: ShareSettings| {
            let nid = notebook_id();
            if nid.is_empty() {
                return;
            }
            let token = auth.token.get();
            if token.is_none() {
                return;
            }

            let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());

            spawn(async move {
                if draft.share_token.is_empty() {
                    // Create share
                    match client
                        .create_share_with_options(
                            &nid,
                            if draft.access_level == "private" {
                                "viewer"
                            } else {
                                &draft.access_level
                            },
                            draft.expires_at.clone(),
                        )
                        .await
                    {
                        Ok(resp) => {
                            let new_settings = ShareSettings {
                                share_token: resp.share_token,
                                access_level: if draft.access_level == "private" {
                                    "link".to_string()
                                } else {
                                    draft.access_level.clone()
                                },
                                expires_at: draft.expires_at.clone(),
                                allow_download: draft.allow_download,
                            };
                            match client.update_share_settings(&nid, &new_settings).await {
                                Ok(updated) => set_settings.set(Some(updated)),
                                Err(error) => set_error.set(format!(
                                    "{}: {}",
                                    choose(
                                        locale.get_untracked(),
                                        "更新分享设置失败",
                                        "Failed to update settings"
                                    ),
                                    error
                                )),
                            }
                        }
                        Err(e) => {
                            set_error.set(format!(
                                "{}: {}",
                                choose(
                                    locale.get_untracked(),
                                    "启用分享失败",
                                    "Failed to enable sharing"
                                ),
                                e
                            ));
                        }
                    }
                } else {
                    match client.revoke_share(&nid, &draft.share_token).await {
                        Ok(_) => match client
                            .update_share_settings(
                                &nid,
                                &ShareSettings {
                                    share_token: String::new(),
                                    access_level: "private".to_string(),
                                    expires_at: None,
                                    allow_download: draft.allow_download,
                                },
                            )
                            .await
                        {
                            Ok(updated) => set_settings.set(Some(updated)),
                            Err(e) => {
                                set_error.set(format!(
                                    "{}: {}",
                                    choose(
                                        locale.get_untracked(),
                                        "关闭分享失败",
                                        "Failed to disable sharing"
                                    ),
                                    e
                                ));
                            }
                        },
                        Err(e) => {
                            set_error.set(format!(
                                "{}: {}",
                                choose(
                                    locale.get_untracked(),
                                    "关闭分享失败",
                                    "Failed to disable sharing"
                                ),
                                e
                            ));
                        }
                    }
                }
            });
        });

    view! {
        <div class=shared_page_style::page_shell>
            <div class=shared_page_style::page_inner>
                <div class=shared_page_style::page_stack>
                <div class=shared_page_style::page_heading_row>
                    <div class=shared_page_style::page_heading>
                        <A href=move || workspace_href.get() attr:class=shared_page_style::back_link>
                            <svg class=shared_page_style::back_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"/>
                            </svg>
                            {move || choose(locale.get(), "返回", "Back")}
                        </A>
                        <h1 class=shared_page_style::page_title>
                            {move || choose(locale.get(), "分享设置", "Share Settings")}
                        </h1>
                        <p class=shared_page_style::page_subtitle>
                            {move || choose(locale.get(), "管理公开访问、成员协作和外部访问反馈。", "Manage external access, collaborators, and share activity from one place.")}
                        </p>
                    </div>
                </div>

                <Show when=move || !error.get().is_empty()>
                    <NoticeBanner message=error.get() tone=NoticeTone::Danger />
                </Show>

                <Show when=move || loading.get()>
                    <div class=shared_page_style::loading_state>
                        {move || choose(locale.get(), "加载中...", "Loading...")}
                    </div>
                </Show>

                <Show when=move || !loading.get()>
                    <div class=shared_page_style::tab_bar>
                        <button
                            class=shared_page_style::tab_button
                            class=(shared_page_style::tab_button_active, move || active_tab.get() == ShareTab::Settings)
                            on:click=move |_| handle_tab_change.with_value(|callback| callback(ShareTab::Settings))
                        >
                            {move || choose(locale.get(), "设置", "Settings")}
                        </button>
                        <button
                            class=shared_page_style::tab_button
                            class=(shared_page_style::tab_button_active, move || active_tab.get() == ShareTab::Analytics)
                            on:click=move |_| handle_tab_change.with_value(|callback| callback(ShareTab::Analytics))
                        >
                            {move || choose(locale.get(), "分析", "Analytics")}
                        </button>
                        <button
                            class=shared_page_style::tab_button
                            class=(shared_page_style::tab_button_active, move || active_tab.get() == ShareTab::AccessLogs)
                            on:click=move |_| handle_tab_change.with_value(|callback| callback(ShareTab::AccessLogs))
                        >
                            {move || choose(locale.get(), "访问日志", "Access Logs")}
                        </button>
                    </div>

                    <ShareCenterPanels
                        active_tab=active_tab
                        locale=locale
                        settings=settings
                        analytics=analytics
                        logs=logs
                        members=members
                        invite_email=invite_email
                        set_invite_email=set_invite_email
                        invite_role=invite_role
                        set_invite_role=set_invite_role
                        inviting=inviting
                        on_settings_updated=settings_callback.clone()
                        on_enable_toggle=toggle_callback.clone()
                        on_invite=invite_callback.clone()
                        set_remove_member_id=set_pending_remove_member_id
                    />
                </Show>
                </div>
            </div>
        </div>
    }
}

// ----------------------------------------------------------------------------
// SharedKbPage - Public access to shared notebook via token
// ----------------------------------------------------------------------------
