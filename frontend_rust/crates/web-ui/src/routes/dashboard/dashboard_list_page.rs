#[component]
pub fn DashboardListPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let location = use_location();
    let is_preview_route =
        Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let is_preview_for_dashboard_home = is_preview_route.clone();
    let dashboard_home_href = Memo::new(move |_| {
        if is_preview_for_dashboard_home.get() {
            "/preview/live/dashboard".to_string()
        } else {
            "/dashboard".to_string()
        }
    });
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
    let (loaded_for_token, set_loaded_for_token) = signal(String::new());
    let (favorite_notebook_ids, set_favorite_notebook_ids) = signal(Vec::<String>::new());
    let (prefs_loaded, set_prefs_loaded) = signal(false);
    let (loaded_prefs_key, set_loaded_prefs_key) = signal(String::new());

    let (create_name, set_create_name) = signal(String::new());
    let (create_description, set_create_description) = signal(String::new());
    let (create_loading, set_create_loading) = signal(false);
    let (create_error, set_create_error) = signal(String::new());

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
                            choose(locale_now, "加载笔记本失败", "Failed to load notebooks"),
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
                                choose(locale_now, "加载账户偏好失败", "Failed to load account preferences"),
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
    let toggle_notebook_favorite = StoredValue::new(
        Arc::new(toggle_notebook_favorite) as Arc<dyn Fn(String) + Send + Sync>
    );

    let rename_notebook = {
        let auth = auth.clone();
        move |notebook_id: String, current_title: String, current_description: String| {
            let Some(next_title) = prompt_notebook_title(&current_title) else {
                return;
            };
            let next_title = next_title.trim().to_string();
            if next_title.is_empty() || next_title == current_title {
                return;
            }

            let Some(token) = auth.token.get() else {
                set_error.set(choose(locale.get_untracked(), "请先登录", "Please sign in first").to_string());
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
                            if let Some(existing) = items.iter_mut().find(|item| item.id == notebook_id) {
                                *existing = resp.notebook;
                            }
                        });
                    }
                    Err(update_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(locale_now, "重命名笔记本失败", "Failed to rename notebook"),
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
                .unwrap_or_else(|| choose(locale.get_untracked(), "未命名笔记本", "Untitled notebook").to_string());

            if !confirm_notebook_delete(&notebook_title) {
                return;
            }

            let Some(token) = auth.token.get() else {
                set_error.set(choose(locale.get_untracked(), "请先登录", "Please sign in first").to_string());
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
                            choose(locale_now, "删除笔记本失败", "Failed to delete notebook"),
                            delete_error
                        ));
                    }
                }
            });
        }
    };
    let delete_notebook = StoredValue::new(
        Arc::new(delete_notebook) as Arc<dyn Fn(String) + Send + Sync>
    );

    let handle_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        let name_val = create_name.get();
        let locale_now = locale.get_untracked();

        if name_val.trim().is_empty() {
            set_create_error
                .set(choose(locale_now, "名称不能为空", "Name is required").to_string());
            return;
        }

        let token = auth.token.get();
        if token.is_none() {
            set_create_error
                .set(choose(locale_now, "尚未登录", "Not authenticated").to_string());
            return;
        }

        set_create_loading.set(true);
        set_create_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token.unwrap());
        let req = CreateNotebookRequest {
            name: name_val.trim().to_string(),
            description: create_description.get().trim().to_string(),
        };

        spawn(async move {
            match client.create_notebook(&req).await {
                Ok(resp) => {
                    set_notebooks.update(|list| {
                        list.insert(0, resp.notebook);
                    });
                    set_create_name.set(String::new());
                    set_create_description.set(String::new());
                    set_show_create_modal.set(false);
                }
                Err(create_err) => {
                    set_create_error.set(format!(
                        "{}: {}",
                        choose(locale_now, "创建笔记本失败", "Failed to create notebook"),
                        create_err
                    ));
                }
            }
            set_create_loading.set(false);
        });
    };

    let current_user_id = Signal::derive(move || {
        auth.user
            .get()
            .map(|user| user.id.clone())
            .unwrap_or_default()
    });

    let notebook_count = Signal::derive(move || {
        let user_id = current_user_id.get();
        notebooks
            .get()
            .into_iter()
            .filter(|notebook| match active_tab.get() {
                DashboardTab::All => true,
                DashboardTab::Mine => notebook.owner_id == user_id,
            })
            .count()
    });

    let visible_notebooks = Signal::derive(move || {
        let user_id = current_user_id.get();
        let mut items = notebooks
            .get()
            .into_iter()
            .filter(|notebook| match active_tab.get() {
                DashboardTab::All => true,
                DashboardTab::Mine => notebook.owner_id == user_id,
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

        view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-[1280px] space-y-6">
                <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-border bg-card px-6 py-5 shadow-sm">
                    <A href=move || dashboard_home_href.get() attr:class="inline-flex items-center gap-3">
                        <span class="inline-flex h-10 w-10 items-center justify-center rounded-2xl border border-border bg-foreground text-background shadow-sm">
                            <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M4 5h16v14H4z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M8 9h8M8 13h5"/>
                            </svg>
                        </span>
                        <span class="text-[1.9rem] font-semibold leading-none tracking-[-0.02em] text-foreground">
                            {"NotebookLM"}
                        </span>
                    </A>

                    <div class="flex items-center gap-3">
                        <A
                            href=move || settings_appearance_href.get()
                            attr:class="app-button-secondary inline-flex items-center gap-2"
                        >
                            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15a3 3 0 100-6 3 3 0 000 6z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33h.01A1.65 1.65 0 009 3.09V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51h.01a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82v.01a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/>
                            </svg>
                            <span>{move || choose(locale.get(), "设置", "Settings")}</span>
                        </A>
                        <A
                            href=move || settings_profile_href.get()
                            attr:class="inline-flex h-11 w-11 items-center justify-center rounded-full border border-border bg-card text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                        >
                            <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 12a4 4 0 100-8 4 4 0 000 8z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 20a8 8 0 0116 0"/>
                            </svg>
                        </A>
                    </div>
                </div>

                <div class="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
                    <nav class="app-tab-bar w-full xl:w-auto">
                        <button
                            type="button"
                            class="app-tab-button"
                            class=("app-tab-button-active", move || active_tab.get() == DashboardTab::All)
                            on:click=move |_| set_active_tab.set(DashboardTab::All)
                        >
                            {move || choose(locale.get(), "全部", "All")}
                        </button>
                        <button
                            type="button"
                            class="app-tab-button"
                            class=("app-tab-button-active", move || active_tab.get() == DashboardTab::Mine)
                            on:click=move |_| set_active_tab.set(DashboardTab::Mine)
                        >
                            {move || choose(locale.get(), "我的笔记本", "My notebooks")}
                        </button>
                    </nav>

                    <div class="flex flex-wrap items-center justify-end gap-3">
                        <button
                            type="button"
                            class="app-button-secondary h-12 w-12 px-0"
                            title={move || choose(locale.get(), "搜索", "Search")}
                        >
                            <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-4.35-4.35m1.85-5.15a7 7 0 11-14 0 7 7 0 0114 0z"/>
                            </svg>
                        </button>

                        <div class="flex items-center rounded-full border border-border bg-card/95 p-1 shadow-sm">
                            <button
                                type="button"
                                class="flex h-10 w-10 items-center justify-center rounded-full transition-colors"
                                class=("bg-background text-foreground shadow-sm", move || view_mode.get() == ViewMode::Card)
                                class=("text-muted-foreground hover:text-foreground", move || view_mode.get() != ViewMode::Card)
                                on:click=move |_| set_view_mode.set(ViewMode::Card)
                                title={move || choose(locale.get(), "卡片视图", "Card view")}
                            >
                                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 5h6v6H4V5zm10 0h6v6h-6V5zM4 13h6v6H4v-6zm10 0h6v6h-6v-6z"/>
                                </svg>
                            </button>
                            <button
                                type="button"
                                class="flex h-10 w-10 items-center justify-center rounded-full transition-colors"
                                class=("bg-background text-foreground shadow-sm", move || view_mode.get() == ViewMode::List)
                                class=("text-muted-foreground hover:text-foreground", move || view_mode.get() != ViewMode::List)
                                on:click=move |_| set_view_mode.set(ViewMode::List)
                                title={move || choose(locale.get(), "列表视图", "List view")}
                            >
                                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 7h14M5 12h14M5 17h14"/>
                                </svg>
                            </button>
                        </div>

                        <Show when=move || sort_menu_open.get()>
                            <button
                                type="button"
                                class="fixed inset-0 z-10 bg-transparent"
                                aria-label={move || choose(locale.get(), "关闭排序菜单", "Close sort menu")}
                                on:click=move |_| set_sort_menu_open.set(false)
                            />
                        </Show>

                        <div class="relative z-20">
                            <button
                                type="button"
                                class="app-button-secondary h-12 min-w-[12rem] justify-between gap-2 px-5"
                                on:click=move |_| set_sort_menu_open.update(|open| *open = !*open)
                            >
                                {move || {
                                    match sort_by.get() {
                                        DashboardSort::Recent => choose(locale.get(), "最近", "Recent"),
                                        DashboardSort::Title => choose(locale.get(), "标题", "Title"),
                                    }
                                }}
                                <svg class="h-4 w-4 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                                </svg>
                            </button>

                            <Show when=move || sort_menu_open.get()>
                                <div class="absolute right-0 top-14 z-20 min-w-[12rem] rounded-2xl border border-border bg-card p-1.5 shadow-lg">
                                    <button
                                        type="button"
                                        class="block w-full rounded-xl px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
                                        on:click=move |_| {
                                            set_sort_by.set(DashboardSort::Recent);
                                            set_sort_menu_open.set(false);
                                        }
                                    >
                                        {move || choose(locale.get(), "最近", "Recent")}
                                    </button>
                                    <button
                                        type="button"
                                        class="block w-full rounded-xl px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
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
                            class="app-button-primary h-12 gap-2 px-6"
                            on:click=move |_| set_show_create_modal.set(true)
                        >
                            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M12 4v16m8-8H4"/>
                            </svg>
                            {move || choose(locale.get(), "新建", "New")}
                        </button>
                    </div>
                </div>

                <div class="flex flex-wrap items-end justify-between gap-4">
                    <div class="app-page-heading mb-0">
                        <h1 class="app-page-title">
                            {move || {
                                if active_tab.get() == DashboardTab::Mine {
                                    choose(locale.get(), "我的笔记本", "My Notebooks")
                                } else {
                                    choose(locale.get(), "全部笔记本", "All Notebooks")
                                }
                            }}
                        </h1>
                    </div>
                    <span class="text-sm text-muted-foreground">
                        {move || format!("{} {}", notebook_count.get(), choose(locale.get(), "个笔记本", "notebooks"))}
                    </span>
                </div>

                <Show when=move || !error.get().is_empty()>
                    <NoticeBanner message={error.get()} tone=NoticeTone::Danger />
                </Show>

                <Show when=move || loading.get()>
                    <div class="app-empty-state">
                        {move || choose(locale.get(), "正在加载笔记本...", "Loading notebooks...")}
                    </div>
                </Show>

                <Show when=move || !loading.get() && notebook_count.get() == 0 && error.get().is_empty()>
                    <div class="app-empty-state">
                        <p class="mb-4">{move || choose(locale.get(), "还没有笔记本", "No notebooks yet")}</p>
                        <button
                            class="app-button-primary"
                            on:click=move |_| set_show_create_modal.set(true)
                        >
                            {move || choose(locale.get(), "创建第一个笔记本", "Create Your First Notebook")}
                        </button>
                    </div>
                </Show>

                <Show when=move || !loading.get() && (notebook_count.get() > 0) && view_mode.get() == ViewMode::Card>
                    {move || {
                        let items = visible_notebooks.get();
                        notebook_card_sections(
                            locale,
                            items,
                            workspace_href_base.get(),
                            favorite_notebook_ids.get(),
                            toggle_notebook_favorite,
                            rename_notebook,
                            delete_notebook,
                        )
                    }}
                </Show>

                <Show when=move || !loading.get() && (notebook_count.get() > 0) && view_mode.get() == ViewMode::List>
                    {move || {
                        let items = visible_notebooks.get();
                        view! {
                            <div class="app-table-shell">
                                <div class="grid grid-cols-12 gap-4 border-b border-border bg-muted/35 px-5 py-3.5 text-sm font-medium text-muted-foreground">
                                    <div class="col-span-6">{move || choose(locale.get(), "标题", "Title")}</div>
                                    <div class="col-span-2">{move || choose(locale.get(), "来源", "Sources")}</div>
                                    <div class="col-span-2">{move || choose(locale.get(), "创建日期", "Created")}</div>
                                    <div class="col-span-2">{move || choose(locale.get(), "角色", "Role")}</div>
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
            </div>
        </div>

        <Show when=move || show_create_modal.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center bg-foreground/55 px-4 backdrop-blur-[2px]">
                <div class="app-surface-card mx-4 w-full max-w-md space-y-4">
                    <h2 class="text-xl font-semibold text-card-foreground">
                        {move || choose(locale.get(), "新建笔记本", "Create New Notebook")}
                    </h2>

                    <form on:submit=handle_create class="space-y-4">
                        <div>
                            <label class="app-form-label" for="notebook-name">
                                {move || choose(locale.get(), "名称 *", "Name *")}
                            </label>
                            <input
                                id="notebook-name"
                                type="text"
                                class="app-input"
                                placeholder={move || choose(locale.get(), "我的笔记本", "my-notebook")}
                                value=create_name.get()
                                on:input=move |ev| set_create_name.set(event_target_value(&ev))
                                required
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
                            <div class="text-sm text-destructive">{create_error.get()}</div>
                        </Show>

                        <div class="flex justify-end gap-3 pt-2">
                            <button
                                type="button"
                                class="app-button-secondary"
                                on:click=move |_| {
                                    set_show_create_modal.set(false);
                                    set_create_error.set(String::new());
                                    set_create_name.set(String::new());
                                    set_create_description.set(String::new());
                                }
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
                                        choose(locale.get(), "创建笔记本", "Create Notebook")
                                    }
                                }}
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </Show>
    }
}
