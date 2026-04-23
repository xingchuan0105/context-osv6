#[component]
pub fn DashboardListPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let location = use_location();
    let navigate = use_navigate();
    let is_preview_route = Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let is_preview_for_settings_appearance = is_preview_route.clone();
    let settings_appearance_href = Memo::new(move |_| {
        if is_preview_for_settings_appearance.get() {
            "/preview/live/settings?tab=appearance".to_string()
        } else {
            "/settings?tab=appearance".to_string()
        }
    });
    let is_preview_for_settings_profile = is_preview_route.clone();
    let settings_profile_href = Memo::new(move |_| {
        if is_preview_for_settings_profile.get() {
            "/preview/live/settings?tab=profile".to_string()
        } else {
            "/settings?tab=profile".to_string()
        }
    });
    let is_preview_for_workspace_base = is_preview_route.clone();
    let workspace_href_base = Memo::new(move |_| {
        if is_preview_for_workspace_base.get() {
            "/preview/live/workspace".to_string()
        } else {
            "/dashboard".to_string()
        }
    });

    let (notebooks, set_notebooks) = signal(Vec::<Notebook>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (view_mode, set_view_mode) = signal(ViewMode::List);
    let (active_tab, set_active_tab) = signal(DashboardTab::Mine);
    let (sort_by, set_sort_by) = signal(DashboardSort::Recent);
    let (sort_menu_open, set_sort_menu_open) = signal(false);
    let (show_create_modal, set_show_create_modal) = signal(false);
    let (show_search_modal, set_show_search_modal) = signal(false);
    let (search_query, set_search_query) = signal(String::new());
    let (loaded_for_token, set_loaded_for_token) = signal(String::new());
    let (favorite_notebook_ids, set_favorite_notebook_ids) = signal(Vec::<String>::new());
    let (prefs_loaded, set_prefs_loaded) = signal(false);
    let (loaded_prefs_key, set_loaded_prefs_key) = signal(String::new());

    let (create_name, set_create_name) = signal(String::new());
    let (create_description, set_create_description) = signal(String::new());
    let (generated_workspace_name, set_generated_workspace_name) = signal(String::new());
    let (generated_workspace_counter_key, set_generated_workspace_counter_key) =
        signal(String::new());
    let (create_loading, set_create_loading) = signal(false);
    let (create_error, set_create_error) = signal(String::new());
    let (selected_search_index, set_selected_search_index) = signal(0_usize);
    let search_input_ref = NodeRef::<leptos::html::Input>::new();

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || auth_for_load.token.get().unwrap_or_default(),
        loaded_for_token,
        set_loaded_for_token,
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };

            set_loading.set(true);
            set_error.set(String::new());
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let locale_now = locale.get_untracked();

            spawn(async move {
                match client.list_notebooks().await {
                    Ok(resp) => set_notebooks.set(resp.notebooks),
                    Err(fetch_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale_now,
                                "加载 Workspace 失败",
                                "Failed to load workspaces"
                            ),
                            fetch_error
                        ));
                    }
                }
                set_loading.set(false);
            });
        },
    );

    run_once_after_hydration(
        || "dashboard-prefs".to_string(),
        loaded_prefs_key,
        set_loaded_prefs_key,
        move || {
            set_favorite_notebook_ids.set(read_favorite_notebook_ids());
            if let Some(token) = auth.token.get_untracked() {
                let client = ApiClient::new(api_base_url()).with_auth(token);
                let locale_now = locale.get_untracked();
                spawn(async move {
                    match client.get_user_preferences().await {
                        Ok(preferences) => {
                            let favorites = preferences.dashboard.favorite_notebook_ids;
                            write_favorite_notebook_ids(&favorites);
                            set_favorite_notebook_ids.set(favorites);
                        }
                        Err(fetch_error) => {
                            set_error.set(format!(
                                "{}: {}",
                                choose(
                                    locale_now,
                                    "加载账户偏好失败",
                                    "Failed to load account preferences"
                                ),
                                fetch_error
                            ));
                        }
                    }
                });
            }
            set_prefs_loaded.set(true);
        },
    );

    Effect::new(move |_| {
        if !prefs_loaded.get() {
            return;
        }
        write_favorite_notebook_ids(&favorite_notebook_ids.get());
    });

    Effect::new(move |_| {
        if !show_search_modal.get() {
            return;
        }

        set_selected_search_index.set(0);

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(input) = search_input_ref.get() {
                let _ = input.focus();
            }
        }
    });

    let toggle_notebook_favorite = {
        let auth = auth.clone();
        move |notebook_id: String| {
            set_favorite_notebook_ids.update(|items| {
                toggle_favorite_notebook_id(items, &notebook_id);
            });
            sync_favorite_notebooks_remote(
                auth.token.get(),
                favorite_notebook_ids.get_untracked(),
                locale.get_untracked(),
                set_error,
            );
        }
    };
    let toggle_notebook_favorite =
        StoredValue::new(Arc::new(toggle_notebook_favorite) as Arc<dyn Fn(String) + Send + Sync>);

    let rename_notebook = {
        let auth = auth.clone();
        move |notebook_id: String, current_title: String, current_description: String| {
            let Some(next_title) = prompt_workspace_title(&current_title) else {
                return;
            };
            let next_title = next_title.trim().to_string();
            if next_title.is_empty() || next_title == current_title {
                return;
            }

            let Some(token) = auth.token.get() else {
                set_error.set(
                    choose(locale.get_untracked(), "请先登录", "Please sign in first").to_string(),
                );
                return;
            };

            let client = ApiClient::new(api_base_url()).with_auth(token);
            let locale_now = locale.get_untracked();
            spawn(async move {
                match client
                    .update_notebook(
                        &notebook_id,
                        &UpdateNotebookRequest {
                            name: next_title,
                            description: current_description,
                        },
                    )
                    .await
                {
                    Ok(resp) => {
                        set_notebooks.update(|items| {
                            if let Some(existing) =
                                items.iter_mut().find(|item| item.id == notebook_id)
                            {
                                *existing = resp.notebook;
                            }
                        });
                    }
                    Err(update_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale_now,
                                "重命名 Workspace 失败",
                                "Failed to rename workspace"
                            ),
                            update_error
                        ));
                    }
                }
            });
        }
    };
    let rename_notebook = StoredValue::new(
        Arc::new(rename_notebook) as Arc<dyn Fn(String, String, String) + Send + Sync>
    );

    let delete_notebook = {
        let auth = auth.clone();
        move |notebook_id: String| {
            let notebook_title = notebooks
                .get_untracked()
                .into_iter()
                .find(|item| item.id == notebook_id)
                .map(|item| {
                    if item.title.trim().is_empty() {
                        item.name
                    } else {
                        item.title
                    }
                })
                .unwrap_or_else(|| {
                    choose(
                        locale.get_untracked(),
                        "未命名 Workspace",
                        "Untitled workspace",
                    )
                    .to_string()
                });

            if !confirm_workspace_delete(&notebook_title) {
                return;
            }

            let Some(token) = auth.token.get() else {
                set_error.set(
                    choose(locale.get_untracked(), "请先登录", "Please sign in first").to_string(),
                );
                return;
            };

            let locale_now = locale.get_untracked();
            let client = ApiClient::new(api_base_url()).with_auth(token.clone());

            spawn(async move {
                match client.delete_notebook(&notebook_id).await {
                    Ok(_) => {
                        set_notebooks.update(|items| items.retain(|item| item.id != notebook_id));

                        let mut next_favorites = favorite_notebook_ids.get_untracked();
                        next_favorites.retain(|id| id != &notebook_id);
                        set_favorite_notebook_ids.set(next_favorites.clone());
                        sync_favorite_notebooks_remote(
                            Some(token),
                            next_favorites,
                            locale_now,
                            set_error,
                        );
                    }
                    Err(delete_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale_now,
                                "删除 Workspace 失败",
                                "Failed to delete workspace"
                            ),
                            delete_error
                        ));
                    }
                }
            });
        }
    };
    let delete_notebook =
        StoredValue::new(Arc::new(delete_notebook) as Arc<dyn Fn(String) + Send + Sync>);
    let navigate_to_workspace = StoredValue::new({
        let navigate = navigate.clone();
        let workspace_href_base = workspace_href_base;
        Arc::new(move |workspace_id: String| {
            navigate(
                &format!("{}/{}", workspace_href_base.get_untracked(), workspace_id),
                leptos_router::NavigateOptions::default(),
            );
        }) as Arc<dyn Fn(String) + Send + Sync>
    });

    let open_create_modal = Arc::new(move || {
        let (default_name, key) = workspace_default_title_for_now(locale.get_untracked());
        set_create_name.set(default_name.clone());
        set_generated_workspace_name.set(default_name);
        set_generated_workspace_counter_key.set(key);
        set_create_description.set(String::new());
        set_create_error.set(String::new());
        set_sort_menu_open.set(false);
        set_show_search_modal.set(false);
        set_show_create_modal.set(true);
    });
    let open_create_modal = StoredValue::new(open_create_modal as Arc<dyn Fn() + Send + Sync>);

    let close_create_modal = Arc::new(move || {
        set_show_create_modal.set(false);
        set_create_error.set(String::new());
        set_create_name.set(String::new());
        set_create_description.set(String::new());
        set_generated_workspace_name.set(String::new());
        set_generated_workspace_counter_key.set(String::new());
    });
    let close_create_modal = StoredValue::new(close_create_modal as Arc<dyn Fn() + Send + Sync>);

    let handle_create = StoredValue::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        let generated_name = generated_workspace_name.get_untracked();
        let generated_key = generated_workspace_counter_key.get_untracked();
        let name_val = create_name.get().trim().to_string();
        let final_name = if name_val.is_empty() {
            generated_name.clone()
        } else {
            name_val
        };
        let locale_now = locale.get_untracked();

        if final_name.is_empty() {
            set_create_error.set(
                choose(
                    locale_now,
                    "无法生成 Workspace 名称",
                    "Workspace name is unavailable",
                )
                .to_string(),
            );
            return;
        }

        let token = auth.token.get();
        if token.is_none() {
            set_create_error.set(choose(locale_now, "尚未登录", "Not authenticated").to_string());
            return;
        }

        set_create_loading.set(true);
        set_create_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let req = CreateNotebookRequest {
            name: final_name.clone(),
            description: create_description.get().trim().to_string(),
        };

        spawn(async move {
            match client.create_notebook(&req).await {
                Ok(resp) => {
                    let workspace_id = resp.notebook.id.clone();
                    set_notebooks.update(|list| {
                        list.insert(0, resp.notebook.clone());
                    });
                    if !generated_name.is_empty()
                        && generated_name == final_name
                        && !generated_key.is_empty()
                    {
                        bump_workspace_default_title_counter(&generated_key);
                    }
                    close_create_modal.with_value(|callback| callback());
                    navigate_to_workspace.with_value(|callback| callback(workspace_id));
                }
                Err(create_err) => {
                    set_create_error.set(format!(
                        "{}: {}",
                        choose(
                            locale_now,
                            "创建 Workspace 失败",
                            "Failed to create workspace"
                        ),
                        create_err
                    ));
                }
            }
            set_create_loading.set(false);
        });
    });

    let current_user_id = Signal::derive(move || {
        auth.user
            .get()
            .map(|user| user.id.clone())
            .unwrap_or_default()
    });

    let notebook_count = Signal::derive(move || {
        let user_id = current_user_id.get();
        let favorites = favorite_notebook_ids.get();
        notebooks
            .get()
            .into_iter()
            .filter(|notebook| match active_tab.get() {
                DashboardTab::All => true,
                DashboardTab::Mine => notebook.owner_id == user_id,
                DashboardTab::Favorites => favorites.iter().any(|item| item == &notebook.id),
            })
            .count()
    });

    let visible_notebooks = Signal::derive(move || {
        let user_id = current_user_id.get();
        let favorites = favorite_notebook_ids.get();
        let mut items = notebooks
            .get()
            .into_iter()
            .filter(|notebook| match active_tab.get() {
                DashboardTab::All => true,
                DashboardTab::Mine => notebook.owner_id == user_id,
                DashboardTab::Favorites => favorites.iter().any(|item| item == &notebook.id),
            })
            .collect::<Vec<_>>();

        match sort_by.get() {
            DashboardSort::Recent => {
                items.sort_by(|left, right| {
                    right
                        .updated_at
                        .cmp(&left.updated_at)
                        .then_with(|| left.title.cmp(&right.title))
                });
            }
            DashboardSort::Title => {
                items.sort_by(|left, right| {
                    left.title
                        .to_lowercase()
                        .cmp(&right.title.to_lowercase())
                        .then_with(|| right.updated_at.cmp(&left.updated_at))
                });
            }
        }

        items
    });

    let search_results = Signal::derive(move || {
        let query = search_query.get().trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }

        let mut results = notebooks
            .get()
            .into_iter()
            .filter(|workspace| {
                let title = dashboard_workspace_display_title(workspace).to_lowercase();
                let description = workspace.description.to_lowercase();
                title.contains(&query) || description.contains(&query)
            })
            .collect::<Vec<_>>();

        results.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.title.cmp(&right.title))
        });
        results
    });

    Effect::new(move |_| {
        let results_len = search_results.get().len();
        let selected_index = selected_search_index.get();
        if results_len == 0 || selected_index >= results_len {
            set_selected_search_index.set(0);
        }
    });

    view! {
        <div class=dashboard_style::shell>
            <header class=dashboard_style::header>
                <div class=dashboard_style::header_inner>
                    <div class=dashboard_style::brand>
                        <span class=dashboard_style::brand_mark>
                            <ContextOsMark class=dashboard_style::brand_mark_icon />
                        </span>
                        <span class=dashboard_style::brand_text>{"Context-OS"}</span>
                    </div>

                    <div class=dashboard_style::header_controls>
                        <A
                            href=move || settings_appearance_href.get()
                            attr:class=dashboard_style::header_button
                        >
                            <svg class=dashboard_style::header_button_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15a3 3 0 100-6 3 3 0 000 6z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33h.01A1.65 1.65 0 009 3.09V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51h.01a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82v.01a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/>
                            </svg>
                            <span>{move || choose(locale.get(), "设置", "Settings")}</span>
                        </A>
                        <A
                            href=move || settings_profile_href.get()
                            attr:class=dashboard_style::avatar_button
                        >
                            <svg class=dashboard_style::avatar_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.15" d="M12 12a4 4 0 100-8 4 4 0 000 8z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.15" d="M4 20a8 8 0 0116 0"/>
                            </svg>
                        </A>
                    </div>
                </div>
            </header>

            <main class=dashboard_style::main>
                <div class=dashboard_style::toolbar>
                    <nav class=dashboard_style::tabs>
                        <button
                            type="button"
                            class=dashboard_style::tab
                            class=(dashboard_style::tab_active, move || active_tab.get() == DashboardTab::All)
                            on:click=move |_| set_active_tab.set(DashboardTab::All)
                        >
                            {move || choose(locale.get(), "全部", "All")}
                        </button>
                        <button
                            type="button"
                            class=dashboard_style::tab
                            class=(dashboard_style::tab_active, move || active_tab.get() == DashboardTab::Mine)
                            on:click=move |_| set_active_tab.set(DashboardTab::Mine)
                        >
                            {move || choose(locale.get(), "我的 Workspace", "My Workspaces")}
                        </button>
                        <button
                            type="button"
                            class=dashboard_style::tab
                            class=(dashboard_style::tab_active, move || active_tab.get() == DashboardTab::Favorites)
                            on:click=move |_| set_active_tab.set(DashboardTab::Favorites)
                        >
                            {move || choose(locale.get(), "我的收藏", "My Favorites")}
                        </button>
                    </nav>

                    <div class=dashboard_style::controls>
                        <button
                            type="button"
                            class=dashboard_style::icon_button
                            title={move || choose(locale.get(), "搜索 Workspace", "Search Workspace")}
                            on:click=move |_| {
                                set_sort_menu_open.set(false);
                                set_show_create_modal.set(false);
                                set_show_search_modal.set(true);
                            }
                        >
                            <svg class=dashboard_style::search_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.25" d="M21 21l-4.35-4.35m1.85-5.15a7 7 0 11-14 0 7 7 0 0114 0z"/>
                            </svg>
                        </button>

                        <div class=dashboard_style::view_toggle>
                            <button
                                type="button"
                                class=dashboard_style::view_button
                                class=(dashboard_style::view_button_active, move || view_mode.get() == ViewMode::Card)
                                on:click=move |_| set_view_mode.set(ViewMode::Card)
                                title={move || choose(locale.get(), "卡片视图", "Card view")}
                            >
                                <svg class=dashboard_style::view_icon_card fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M4 5h6v6H4V5zm10 0h6v6h-6V5zM4 13h6v6H4v-6zm10 0h6v6h-6v-6z"/>
                                </svg>
                            </button>
                            <button
                                type="button"
                                class=dashboard_style::view_button
                                class=(dashboard_style::view_button_active, move || view_mode.get() == ViewMode::List)
                                on:click=move |_| set_view_mode.set(ViewMode::List)
                                title={move || choose(locale.get(), "列表视图", "List view")}
                            >
                                <svg class=dashboard_style::view_icon_list fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M5 7h14M5 12h14M5 17h14"/>
                                </svg>
                            </button>
                        </div>

                        <Show when=move || sort_menu_open.get()>
                            <button
                                type="button"
                                class=dashboard_style::backdrop
                                aria-label={move || choose(locale.get(), "关闭排序菜单", "Close sort menu")}
                                on:click=move |_| set_sort_menu_open.set(false)
                            />
                        </Show>

                        <div class=dashboard_style::menu_anchor>
                            <button
                                type="button"
                                class=dashboard_style::sort_trigger
                                on:click=move |_| set_sort_menu_open.update(|open| *open = !*open)
                            >
                                <span>
                                    {move || match sort_by.get() {
                                        DashboardSort::Recent => choose(locale.get(), "最近", "Recent"),
                                        DashboardSort::Title => choose(locale.get(), "标题", "Title"),
                                    }}
                                </span>
                                <svg class=dashboard_style::sort_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 9l-7 7-7-7"/>
                                </svg>
                            </button>

                            <Show when=move || sort_menu_open.get()>
                                <div class=dashboard_style::menu>
                                    <button
                                        type="button"
                                        class=dashboard_style::menu_item
                                        on:click=move |_| {
                                            set_sort_by.set(DashboardSort::Recent);
                                            set_sort_menu_open.set(false);
                                        }
                                    >
                                        {move || choose(locale.get(), "最近", "Recent")}
                                    </button>
                                    <button
                                        type="button"
                                        class=dashboard_style::menu_item
                                        on:click=move |_| {
                                            set_sort_by.set(DashboardSort::Title);
                                            set_sort_menu_open.set(false);
                                        }
                                    >
                                        {move || choose(locale.get(), "标题", "Title")}
                                    </button>
                                </div>
                            </Show>
                        </div>

                        <button
                            type="button"
                            class=dashboard_style::primary_button
                            on:click=move |_| open_create_modal.with_value(|callback| callback())
                        >
                            <svg class=dashboard_style::primary_button_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.4" d="M12 4v16m8-8H4"/>
                            </svg>
                            <span>{move || choose(locale.get(), "新建 Workspace", "New Workspace")}</span>
                        </button>
                    </div>
                </div>

                <div class=dashboard_style::heading_row>
                    <h1 class=dashboard_style::heading>
                        {move || {
                            match active_tab.get() {
                                DashboardTab::Mine => choose(locale.get(), "我的 Workspace", "My Workspaces"),
                                DashboardTab::Favorites => choose(locale.get(), "我的收藏", "My Favorites"),
                                DashboardTab::All => choose(locale.get(), "全部 Workspace", "All Workspaces"),
                            }
                        }}
                    </h1>
                    <span class=dashboard_style::heading_meta>
                        {move || format!("{} {}", notebook_count.get(), choose(locale.get(), "个 Workspace", "workspaces"))}
                    </span>
                </div>

                <Show when=move || !error.get().is_empty()>
                    <div class=dashboard_style::error_block>
                        <NoticeBanner message={error.get()} tone=NoticeTone::Danger />
                    </div>
                </Show>

                <Show when=move || loading.get()>
                    <div class=dashboard_style::empty_state>
                        {move || choose(locale.get(), "正在加载 Workspace...", "Loading workspaces...")}
                    </div>
                </Show>

                <Show when=move || !loading.get() && notebook_count.get() == 0 && error.get().is_empty()>
                    <div class=dashboard_style::empty_state>
                        <p>{move || match active_tab.get() {
                            DashboardTab::Favorites => choose(locale.get(), "还没有收藏的 Workspace", "No favorite workspaces yet"),
                            DashboardTab::Mine => choose(locale.get(), "还没有 Workspace", "No workspaces yet"),
                            DashboardTab::All => choose(locale.get(), "还没有 Workspace", "No workspaces yet"),
                        }}</p>
                        <button
                            type="button"
                            class=format!("{} {}", dashboard_style::primary_button, dashboard_style::empty_state_button)
                            on:click=move |_| open_create_modal.with_value(|callback| callback())
                        >
                            {move || choose(locale.get(), "创建第一个 Workspace", "Create your first workspace")}
                        </button>
                    </div>
                </Show>

                <Show when=move || !loading.get() && (notebook_count.get() > 0) && view_mode.get() == ViewMode::Card>
                    {move || {
                        let items = visible_notebooks.get();
                        view! {
                            <div class=dashboard_style::card_grid>
                                <button
                                    type="button"
                                    class=format!("{} {}", dashboard_style::card, dashboard_style::quick_create_card)
                                    on:click=move |_| open_create_modal.with_value(|callback| callback())
                                >
                                    <span class=dashboard_style::quick_create_badge>{"+"}</span>
                                    <div class=dashboard_style::quick_create_body>
                                        <div class=dashboard_style::quick_create_title>
                                            {move || choose(locale.get(), "新建 Workspace", "New Workspace")}
                                        </div>
                                        <div class=dashboard_style::quick_create_hint>
                                            {move || choose(locale.get(), "立即创建并进入新的工作区。", "Create and open a new workspace immediately.")}
                                        </div>
                                    </div>
                                </button>
                                {notebook_card_sections(
                                    locale,
                                    items,
                                    workspace_href_base.get(),
                                    favorite_notebook_ids.get(),
                                    toggle_notebook_favorite,
                                    rename_notebook,
                                    delete_notebook,
                                )}
                            </div>
                        }
                        .into_any()
                    }}
                </Show>

                <Show when=move || !loading.get() && (notebook_count.get() > 0) && view_mode.get() == ViewMode::List>
                    {move || {
                        let items = visible_notebooks.get();
                        view! {
                            <div class=dashboard_style::list_table>
                                <div class=dashboard_style::list_header>
                                    <div class=dashboard_style::list_col_title>{move || choose(locale.get(), "标题", "Title")}</div>
                                    <div class=dashboard_style::list_col_meta>{move || choose(locale.get(), "来源", "Sources")}</div>
                                    <div class=dashboard_style::list_col_meta>{move || choose(locale.get(), "最近更新", "Updated")}</div>
                                    <div class=dashboard_style::list_col_meta>{move || choose(locale.get(), "角色", "Role")}</div>
                                </div>
                                {notebook_list_sections(
                                    locale,
                                    items,
                                    workspace_href_base.get(),
                                    current_user_id.get(),
                                    favorite_notebook_ids.get(),
                                    toggle_notebook_favorite,
                                    rename_notebook,
                                    delete_notebook,
                                )}
                            </div>
                        }
                        .into_any()
                    }}
                </Show>
            </main>
        </div>

        <Show when=move || show_create_modal.get()>
            <div class=dashboard_style::create_modal_overlay>
                <div class=format!("app-surface-card {}", dashboard_style::modal_card)>
                    <h2 class=dashboard_style::modal_title>
                        {move || choose(locale.get(), "新建 Workspace", "Create New Workspace")}
                    </h2>

                    <form
                        on:submit=move |ev| handle_create.with_value(|callback| callback(ev))
                        class=dashboard_style::modal_form
                    >
                        <div>
                            <label class="app-form-label" for="notebook-name">
                                {move || choose(locale.get(), "名称", "Name")}
                            </label>
                            <input
                                id="notebook-name"
                                type="text"
                                class="app-input"
                                placeholder={move || choose(locale.get(), "未命名 Workspace", "Untitled Workspace")}
                                value=create_name.get()
                                on:input=move |ev| set_create_name.set(event_target_value(&ev))
                            />
                        </div>

                        <div>
                            <label class="app-form-label" for="notebook-desc">
                                {move || choose(locale.get(), "描述", "Description")}
                            </label>
                            <textarea
                                id="notebook-desc"
                                class="app-input"
                                rows="3"
                                placeholder={move || choose(locale.get(), "可选描述...", "Optional description...")}
                                on:input=move |ev| set_create_description.set(event_target_value(&ev))
                            ></textarea>
                        </div>

                        <Show when=move || !create_error.get().is_empty()>
                            <div class=dashboard_style::modal_error>{create_error.get()}</div>
                        </Show>

                        <div class=dashboard_style::modal_actions>
                            <button
                                type="button"
                                class="app-button-secondary"
                                on:click=move |_| close_create_modal.with_value(|callback| callback())
                            >
                                {move || choose(locale.get(), "取消", "Cancel")}
                            </button>
                            <button
                                type="submit"
                                class="app-button-primary"
                                disabled=create_loading.get()
                            >
                                {move || {
                                    if create_loading.get() {
                                        choose(locale.get(), "创建中...", "Creating...")
                                    } else {
                                        choose(locale.get(), "创建 Workspace", "Create Workspace")
                                    }
                                }}
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </Show>

        <Show when=move || show_search_modal.get()>
            <div
                class=dashboard_style::search_modal_overlay
                on:click=move |_| set_show_search_modal.set(false)
            >
                <div
                    class=dashboard_style::search_modal_card
                    on:click=move |ev| ev.stop_propagation()
                    on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                        match ev.key().as_str() {
                            "Escape" => {
                                ev.prevent_default();
                                set_show_search_modal.set(false);
                            }
                            "ArrowDown" => {
                                let results_len = search_results.get_untracked().len();
                                if results_len > 0 {
                                    ev.prevent_default();
                                    set_selected_search_index.update(|index| {
                                        *index = (*index + 1).min(results_len.saturating_sub(1));
                                    });
                                }
                            }
                            "ArrowUp" => {
                                if !search_results.get_untracked().is_empty() {
                                    ev.prevent_default();
                                    set_selected_search_index.update(|index| {
                                        *index = index.saturating_sub(1);
                                    });
                                }
                            }
                            "Enter" => {
                                let results = search_results.get_untracked();
                                if let Some(workspace) =
                                    results.get(selected_search_index.get_untracked()).cloned()
                                {
                                    ev.prevent_default();
                                    set_show_search_modal.set(false);
                                    navigate_to_workspace
                                        .with_value(|callback| callback(workspace.id.clone()));
                                }
                            }
                            _ => {}
                        }
                    }
                >
                    <div class=dashboard_style::search_modal_header>
                        <div class=dashboard_style::search_modal_title>
                            {move || choose(locale.get(), "快速打开 Workspace", "Quick open workspaces")}
                        </div>
                        <div class=dashboard_style::search_modal_hint>
                            {move || choose(locale.get(), "输入关键词，点击结果进入 Workspace", "Type a keyword and open a workspace")}
                        </div>
                    </div>

                    <input
                        node_ref=search_input_ref
                        type="text"
                        class=dashboard_style::search_modal_input
                        placeholder={move || choose(locale.get(), "搜索 Workspace 标题或描述", "Search workspace title or description")}
                        value=search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    />

                    <div class=dashboard_style::search_modal_results>
                        {move || {
                            let query = search_query.get();
                            if query.trim().is_empty() {
                                return view! {
                                    <div class=dashboard_style::search_modal_empty>
                                        {move || choose(locale.get(), "输入关键词搜索 Workspace", "Start typing to search workspaces")}
                                    </div>
                                }
                                .into_any();
                            }

                            let results = search_results.get();

                            if results.is_empty() {
                                return view! {
                                    <div class=dashboard_style::search_modal_empty>
                                        {move || choose(locale.get(), "没有匹配的 Workspace", "No matching workspaces")}
                                    </div>
                                }
                                .into_any();
                            }

                            view! {
                                <div class=dashboard_style::search_results_list>
                                    {results
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, workspace)| {
                                            let workspace_id = workspace.id.clone();
                                            let workspace_title = dashboard_workspace_display_title(&workspace);
                                            let workspace_description = workspace.description.clone();
                                            let has_workspace_description = !workspace_description.trim().is_empty();
                                            let workspace_description_for_view = StoredValue::new(workspace_description);
                                            let workspace_date = dashboard_notebook_date_label(locale.get(), &workspace.updated_at);
                                            let result_href = format!("{}/{}", workspace_href_base.get(), workspace_id);
                                            view! {
                                                <A
                                                    href=result_href
                                                    attr:class=move || {
                                                        if selected_search_index.get() == index {
                                                            format!(
                                                                "{} {}",
                                                                dashboard_style::search_result_row,
                                                                dashboard_style::search_result_row_active
                                                            )
                                                        } else {
                                                            dashboard_style::search_result_row.to_string()
                                                        }
                                                    }
                                                    on:mouseenter=move |_| set_selected_search_index.set(index)
                                                    on:click=move |_| set_show_search_modal.set(false)
                                                >
                                                    <div class=dashboard_style::search_result_main>
                                                        <div class=dashboard_style::search_result_title>{workspace_title}</div>
                                                        <Show when=move || has_workspace_description>
                                                            <div class=dashboard_style::search_result_description>
                                                                {move || workspace_description_for_view.with_value(|value| value.clone())}
                                                            </div>
                                                        </Show>
                                                    </div>
                                                    <div class=dashboard_style::search_result_meta>
                                                        {workspace_date}
                                                    </div>
                                                </A>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            }
                            .into_any()
                        }}
                    </div>
                </div>
            </div>
        </Show>
    }
}
