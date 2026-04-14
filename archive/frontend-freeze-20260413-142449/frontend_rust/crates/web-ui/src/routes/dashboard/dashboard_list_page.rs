#[component]
pub fn DashboardListPage() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let query_params = use_query_map();
    let navigate = use_navigate();

    // State signals
    let (notebooks, set_notebooks) = signal(Vec::<Notebook>::new());
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (view_mode, set_view_mode) = signal(ViewMode::List);
    let (active_tab, set_active_tab) = signal(DashboardTab::Mine);
    let (sort_by, set_sort_by) = signal(DashboardSort::Recent);
    let (collection_filter, set_collection_filter) = signal(DashboardCollectionFilter::All);
    let (search_query, set_search_query) = signal(String::new());
    let (show_search_input, set_show_search_input) = signal(false);
    let (sort_menu_open, set_sort_menu_open) = signal(false);
    let (show_create_modal, set_show_create_modal) = signal(false);
    let (loaded_for_token, set_loaded_for_token) = signal(String::new());
    let (favorite_notebook_ids, set_favorite_notebook_ids) = signal(Vec::<String>::new());
    let (prefs_loaded, set_prefs_loaded) = signal(false);
    let (loaded_prefs_key, set_loaded_prefs_key) = signal(String::new());

    // Create form signals
    let (create_name, set_create_name) = signal(String::new());
    let (create_description, set_create_description) = signal(String::new());
    let (create_loading, set_create_loading) = signal(false);
    let (create_error, set_create_error) = signal(String::new());
    let (dashboard_query_loaded, set_dashboard_query_loaded) = signal(false);

    // Restore dashboard query state on first load
    Effect::new(move |_| {
        if dashboard_query_loaded.get() {
            return;
        }
        let params = query_params.get();

        match params
            .get("tab")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "all" => set_active_tab.set(DashboardTab::All),
            "mine" => set_active_tab.set(DashboardTab::Mine),
            _ => {}
        }

        match params
            .get("view")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "card" => set_view_mode.set(ViewMode::Card),
            "list" => set_view_mode.set(ViewMode::List),
            _ => {}
        }

        match params
            .get("sort")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "recent" => set_sort_by.set(DashboardSort::Recent),
            "title" => set_sort_by.set(DashboardSort::Title),
            _ => {}
        }

        match params
            .get("scope")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "all" => set_collection_filter.set(DashboardCollectionFilter::All),
            "favorites" => set_collection_filter.set(DashboardCollectionFilter::Favorited),
            "shared" => set_collection_filter.set(DashboardCollectionFilter::Shared),
            _ => {}
        }

        let initial_query = params.get("q").unwrap_or_default().trim().to_string();
        if !initial_query.is_empty() {
            set_search_query.set(initial_query);
            set_show_search_input.set(true);
        }

        set_dashboard_query_loaded.set(true);
    });

    // Fetch notebooks on hydration
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
                    Ok(resp) => {
                        set_notebooks.set(resp.notebooks);
                    }
                    Err(fetch_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(locale_now, "加载知识库失败", "Failed to load notebooks"),
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
                set_error
                    .set(choose(locale.get_untracked(), "请先登录", "Please sign in first").to_string());
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
                            if let Some(existing) = items.iter_mut().find(|item| item.id == notebook_id)
                            {
                                *existing = resp.notebook;
                            }
                        });
                    }
                    Err(update_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(locale_now, "重命名知识库失败", "Failed to rename notebook"),
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
                .unwrap_or_else(|| choose(locale.get_untracked(), "未命名知识库", "Untitled notebook").to_string());

            if !confirm_notebook_delete(&notebook_title) {
                return;
            }

            let Some(token) = auth.token.get() else {
                set_error
                    .set(choose(locale.get_untracked(), "请先登录", "Please sign in first").to_string());
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
                            choose(locale_now, "删除知识库失败", "Failed to delete notebook"),
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

    // Create notebook handler
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
            set_create_error.set(choose(locale_now, "尚未登录", "Not authenticated").to_string());
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
                        choose(locale_now, "创建知识库失败", "Failed to create notebook"),
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

    let scoped_notebooks = Signal::derive(move || {
        let user_id = current_user_id.get();
        notebooks
            .get()
            .into_iter()
            .filter(|notebook| match active_tab.get() {
                DashboardTab::All => true,
                DashboardTab::Mine => notebook.owner_id == user_id,
            })
            .collect::<Vec<_>>()
    });

    let scope_counts = Signal::derive(move || {
        let favorites = favorite_notebook_ids.get();
        let mut all_count = 0usize;
        let mut favorited_count = 0usize;
        let mut shared_count = 0usize;

        for notebook in scoped_notebooks.get() {
            all_count += 1;
            if favorites.iter().any(|id| id == &notebook.id) {
                favorited_count += 1;
            }
            if notebook.shared {
                shared_count += 1;
            }
        }

        (all_count, favorited_count, shared_count)
    });

    let has_active_filters = Signal::derive(move || {
        !search_query.get().trim().is_empty()
            || collection_filter.get() != DashboardCollectionFilter::All
    });

    let visible_notebooks = Signal::derive(move || {
        let query = search_query.get().trim().to_lowercase();
        let favorites = favorite_notebook_ids.get();
        let mut items = scoped_notebooks
            .get()
            .into_iter()
            .filter(|notebook| match collection_filter.get() {
                DashboardCollectionFilter::All => true,
                DashboardCollectionFilter::Favorited => {
                    favorites.iter().any(|id| id == &notebook.id)
                }
                DashboardCollectionFilter::Shared => notebook.shared,
            })
            .filter(|notebook| {
                if query.is_empty() {
                    return true;
                }

                notebook.title.to_lowercase().contains(&query)
                    || notebook.name.to_lowercase().contains(&query)
                    || notebook.description.to_lowercase().contains(&query)
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

    // Keep dashboard filters in URL query for refresh/share continuity.
    Effect::new(move |_| {
        if !dashboard_query_loaded.get() {
            return;
        }

        let tab = match active_tab.get() {
            DashboardTab::All => "all",
            DashboardTab::Mine => "mine",
        };
        let view = match view_mode.get() {
            ViewMode::Card => "card",
            ViewMode::List => "list",
        };
        let sort = match sort_by.get() {
            DashboardSort::Recent => "recent",
            DashboardSort::Title => "title",
        };
        let scope = match collection_filter.get() {
            DashboardCollectionFilter::All => "all",
            DashboardCollectionFilter::Favorited => "favorites",
            DashboardCollectionFilter::Shared => "shared",
        };
        let query = search_query.get().trim().to_string();

        let params = query_params.get();
        let current_tab = params
            .get("tab")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let current_view = params
            .get("view")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let current_sort = params
            .get("sort")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let current_scope = params
            .get("scope")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let current_query = params.get("q").unwrap_or_default().trim().to_string();

        if current_tab == tab
            && current_view == view
            && current_sort == sort
            && current_scope == scope
            && current_query == query
        {
            return;
        }

        let mut target = format!("/dashboard?tab={tab}&view={view}&sort={sort}&scope={scope}");
        if !query.is_empty() {
            target.push_str("&q=");
            target.push_str(&encode_query_param(&query));
        }
        navigate(&target, NavigateOptions::default());
    });

    view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-7xl">
                <div class="mb-8 flex flex-col gap-5 rounded-2xl border border-border/70 bg-card/40 p-5">
                    <div class="flex flex-wrap items-center justify-between gap-4">
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="rounded-full px-5 py-2 text-sm font-medium transition-colors"
                                class=("bg-muted text-foreground", move || active_tab.get() == DashboardTab::All)
                                class=("text-muted-foreground hover:bg-muted/60", move || active_tab.get() != DashboardTab::All)
                                on:click=move |_| set_active_tab.set(DashboardTab::All)
                            >
                                {move || choose(locale.get(), "全部", "All")}
                            </button>
                            <button
                                type="button"
                                class="rounded-full px-5 py-2 text-sm font-medium transition-colors"
                                class=("bg-muted text-foreground", move || active_tab.get() == DashboardTab::Mine)
                                class=("text-muted-foreground hover:bg-muted/60", move || active_tab.get() != DashboardTab::Mine)
                                on:click=move |_| set_active_tab.set(DashboardTab::Mine)
                            >
                                {move || choose(locale.get(), "我的笔记本", "My notebooks")}
                            </button>
                        </div>

                        <div class="flex flex-wrap items-center justify-end gap-2.5">
                            <button
                                type="button"
                                class="flex h-9 w-9 items-center justify-center rounded-full border border-border bg-background text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                                title={move || choose(locale.get(), "搜索", "Search")}
                                on:click=move |_| {
                                    if show_search_input.get_untracked() {
                                        set_show_search_input.set(false);
                                        set_search_query.set(String::new());
                                    } else {
                                        set_show_search_input.set(true);
                                    }
                                }
                            >
                                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-4.35-4.35m1.85-5.15a7 7 0 11-14 0 7 7 0 0114 0z"/>
                                </svg>
                            </button>

                            <Show when=move || show_search_input.get()>
                                <div class="flex items-center gap-1">
                                    <input
                                        type="search"
                                        class="app-input h-9 w-[180px] rounded-full sm:w-[220px]"
                                        placeholder={move || choose(locale.get(), "搜索笔记本", "Search notebooks")}
                                        prop:value=move || search_query.get()
                                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                                    />
                                    <button
                                        type="button"
                                        class="flex h-8 w-8 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                                        title={move || choose(locale.get(), "清除搜索", "Clear search")}
                                        on:click=move |_| {
                                            set_search_query.set(String::new());
                                            set_show_search_input.set(false);
                                        }
                                    >
                                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                                        </svg>
                                    </button>
                                </div>
                            </Show>

                            <div class="flex items-center rounded-full border border-border bg-muted/60 p-1">
                                <button
                                    type="button"
                                    class="rounded-full px-3 py-1.5 text-sm transition-colors"
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
                                    class="rounded-full px-3 py-1.5 text-sm transition-colors"
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
                                    class="flex items-center gap-2 rounded-full border border-border bg-background px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted"
                                    on:click=move |_| set_sort_menu_open.update(|open| *open = !*open)
                                >
                                    {move || {
                                        match sort_by.get() {
                                            DashboardSort::Recent => choose(locale.get(), "最近", "Recent"),
                                            DashboardSort::Title => choose(locale.get(), "标题", "Title"),
                                        }
                                    }}
                                    <svg class="h-3.5 w-3.5 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                                    </svg>
                                </button>

                                <Show when=move || sort_menu_open.get()>
                                    <div class="absolute right-0 top-11 z-20 w-28 rounded-xl border border-border bg-card p-1 shadow-lg">
                                        <button
                                            type="button"
                                            class="block w-full rounded-lg px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
                                            on:click=move |_| {
                                                set_sort_by.set(DashboardSort::Recent);
                                                set_sort_menu_open.set(false);
                                            }
                                        >
                                            {move || choose(locale.get(), "最近", "Recent")}
                                        </button>
                                        <button
                                            type="button"
                                            class="block w-full rounded-lg px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
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
                                class="app-button-primary flex items-center gap-2 rounded-full"
                                on:click=move |_| set_show_create_modal.set(true)
                            >
                                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>
                                </svg>
                                {move || choose(locale.get(), "新建", "New")}
                            </button>
                        </div>
                    </div>

                </div>

                <div class="mb-6 flex flex-col gap-3">
                    <div class="flex flex-wrap items-center justify-between gap-4">
                        <h1 class="text-2xl font-medium tracking-tight text-foreground">
                            {move || {
                                if active_tab.get() == DashboardTab::Mine {
                                    choose(locale.get(), "我的笔记本", "My Notebooks")
                                } else {
                                    choose(locale.get(), "全部笔记本", "All Notebooks")
                                }
                            }}
                        </h1>

                        <span class="text-sm text-muted-foreground">
                            {move || {
                                let (all_count, _, _) = scope_counts.get();
                                format!("{} {}", all_count, choose(locale.get(), "个笔记本", "notebooks"))
                            }}
                        </span>
                    </div>

                    <div class="flex flex-wrap items-center gap-2">
                        <button
                            type="button"
                            class="rounded-full px-4 py-1.5 text-sm font-medium transition-colors"
                            class=("bg-muted text-foreground", move || collection_filter.get() == DashboardCollectionFilter::All)
                            class=("border border-border/80 text-muted-foreground hover:bg-muted/60", move || collection_filter.get() != DashboardCollectionFilter::All)
                            on:click=move |_| set_collection_filter.set(DashboardCollectionFilter::All)
                        >
                            {move || {
                                let (all_count, _, _) = scope_counts.get();
                                format!("{} ({})", choose(locale.get(), "全部", "All"), all_count)
                            }}
                        </button>

                        <button
                            type="button"
                            class="rounded-full px-4 py-1.5 text-sm font-medium transition-colors"
                            class=("bg-muted text-foreground", move || collection_filter.get() == DashboardCollectionFilter::Favorited)
                            class=("border border-border/80 text-muted-foreground hover:bg-muted/60", move || collection_filter.get() != DashboardCollectionFilter::Favorited)
                            on:click=move |_| set_collection_filter.set(DashboardCollectionFilter::Favorited)
                        >
                            {move || {
                                let (_, favorited_count, _) = scope_counts.get();
                                format!("{} ({})", choose(locale.get(), "收藏", "Favorites"), favorited_count)
                            }}
                        </button>

                        <button
                            type="button"
                            class="rounded-full px-4 py-1.5 text-sm font-medium transition-colors"
                            class=("bg-muted text-foreground", move || collection_filter.get() == DashboardCollectionFilter::Shared)
                            class=("border border-border/80 text-muted-foreground hover:bg-muted/60", move || collection_filter.get() != DashboardCollectionFilter::Shared)
                            on:click=move |_| set_collection_filter.set(DashboardCollectionFilter::Shared)
                        >
                            {move || {
                                let (_, _, shared_count) = scope_counts.get();
                                format!("{} ({})", choose(locale.get(), "共享", "Shared"), shared_count)
                            }}
                        </button>
                    </div>
                </div>

                <Show when=move || !error.get().is_empty()>
                    <NoticeBanner message={error.get()} tone=NoticeTone::Danger />
                </Show>

                <Show when=move || loading.get()>
                    <div class="app-empty-state">
                        {move || choose(locale.get(), "正在加载知识库...", "Loading notebooks...")}
                    </div>
                </Show>

                <Show when=move || !loading.get() && notebooks.get().is_empty() && error.get().is_empty()>
                    <div class="app-empty-state">
                        <p class="mb-4">
                            {move || choose(locale.get(), "还没有知识库", "No notebooks yet")}
                        </p>
                        <button
                            class="app-button-primary"
                            on:click=move |_| set_show_create_modal.set(true)
                        >
                            {move || choose(locale.get(), "创建第一个知识库", "Create Your First Notebook")}
                        </button>
                    </div>
                </Show>

                <Show when=move || !loading.get() && !notebooks.get().is_empty() && view_mode.get() == ViewMode::Card>
                    {move || {
                        let items = visible_notebooks.get();
                        if items.is_empty() {
                            view! {
                                <div class="app-empty-state">
                                    <p class="mb-4">
                                        {move || {
                                            if has_active_filters.get() {
                                                choose(locale.get(), "没有符合当前筛选条件的知识库", "No notebooks match the current filters")
                                            } else {
                                                choose(locale.get(), "当前范围下还没有知识库", "No notebooks in this scope yet")
                                            }
                                        }}
                                    </p>
                                    <Show when=move || has_active_filters.get()>
                                        <button
                                            type="button"
                                            class="app-button-secondary"
                                            on:click=move |_| {
                                                set_collection_filter.set(DashboardCollectionFilter::All);
                                                set_search_query.set(String::new());
                                                set_show_search_input.set(false);
                                            }
                                        >
                                            {move || choose(locale.get(), "清除筛选", "Clear filters")}
                                        </button>
                                    </Show>
                                </div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <div class="grid grid-cols-1 gap-5 pb-8 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                                    <button
                                        type="button"
                                        class="group flex h-[188px] flex-col items-center justify-center rounded-2xl border border-dashed border-border bg-muted/30 text-muted-foreground transition-colors hover:bg-muted"
                                        on:click=move |_| set_show_create_modal.set(true)
                                    >
                                        <div class="mb-3 flex h-11 w-11 items-center justify-center rounded-full border border-border bg-background text-foreground transition-transform group-hover:scale-105">
                                            <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>
                                            </svg>
                                        </div>
                                        <span class="text-sm font-medium">{move || choose(locale.get(), "新建知识库", "New notebook")}</span>
                                    </button>

                                    {notebook_card_sections(
                                        locale,
                                        items,
                                        favorite_notebook_ids.get(),
                                        toggle_notebook_favorite,
                                        rename_notebook,
                                        delete_notebook,
                                    )}
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </Show>

                <Show when=move || !loading.get() && !notebooks.get().is_empty() && view_mode.get() == ViewMode::List>
                    {move || {
                        let items = visible_notebooks.get();
                        if items.is_empty() {
                            view! {
                                <div class="app-empty-state">
                                    <p class="mb-4">
                                        {move || {
                                            if has_active_filters.get() {
                                                choose(locale.get(), "没有符合当前筛选条件的知识库", "No notebooks match the current filters")
                                            } else {
                                                choose(locale.get(), "当前范围下还没有知识库", "No notebooks in this scope yet")
                                            }
                                        }}
                                    </p>
                                    <Show when=move || has_active_filters.get()>
                                        <button
                                            type="button"
                                            class="app-button-secondary"
                                            on:click=move |_| {
                                                set_collection_filter.set(DashboardCollectionFilter::All);
                                                set_search_query.set(String::new());
                                                set_show_search_input.set(false);
                                            }
                                        >
                                            {move || choose(locale.get(), "清除筛选", "Clear filters")}
                                        </button>
                                    </Show>
                                </div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <div class="overflow-hidden rounded-2xl border border-border/80 bg-card">
                                    <div class="grid grid-cols-12 gap-4 border-b border-border/70 px-4 py-3 text-[12px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
                                        <div class="col-span-6">{move || choose(locale.get(), "标题", "Title")}</div>
                                        <div class="col-span-2">{move || choose(locale.get(), "来源", "Sources")}</div>
                                        <div class="col-span-2">{move || choose(locale.get(), "创建日期", "Created")}</div>
                                        <div class="col-span-2">{move || choose(locale.get(), "角色", "Role")}</div>
                                    </div>
                                    {notebook_list_sections(
                                        locale,
                                        items,
                                        current_user_id.get(),
                                        favorite_notebook_ids.get(),
                                        toggle_notebook_favorite,
                                        rename_notebook,
                                        delete_notebook,
                                    )}
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </Show>
            </div>
        </div>

        // Create notebook modal
        <Show when=move || show_create_modal.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
                <div class="app-surface-card mx-4 w-full max-w-md">
                    <h2 class="mb-4 text-xl font-semibold text-card-foreground">
                        {move || choose(locale.get(), "新建知识库", "Create New Notebook")}
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
                                placeholder={move || choose(locale.get(), "我的知识库", "my-notebook")}
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
                            <div class="text-sm text-red-600">{create_error.get()}</div>
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
                                        choose(locale.get(), "创建知识库", "Create Notebook")
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

// ----------------------------------------------------------------------------
// WorkspacePage - Individual notebook workspace with 3-column layout
// ----------------------------------------------------------------------------
