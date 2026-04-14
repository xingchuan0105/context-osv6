#[component]
fn WorkspaceTopBar(
    locale: ReadSignal<crate::i18n::Locale>,
    workspace_id: Memo<String>,
    workspace_name: ReadSignal<String>,
    set_workspace_name: WriteSignal<String>,
    set_workspace_error: WriteSignal<String>,
    set_left_rail_open: WriteSignal<bool>,
    set_right_rail_open: WriteSignal<bool>,
) -> impl IntoView {
    let auth = use_auth_state();
    let navigate = leptos_router::hooks::use_navigate();
    let (is_editing_title, set_is_editing_title) = signal(false);
    let (title_draft, set_title_draft) = signal(String::new());
    let (show_create_modal, set_show_create_modal) = signal(false);
    let (new_notebook_name, set_new_notebook_name) = signal(String::new());
    let (user_menu_open, set_user_menu_open) = signal(false);
    let (creating_notebook, set_creating_notebook) = signal(false);

    Effect::new(move |_| {
        if !is_editing_title.get() {
            set_title_draft.set(workspace_name.get());
        }
    });

    let auth_for_title = auth.clone();
    let save_workspace_title = move || {
        let Some(token) = auth_for_title.token.get_untracked() else {
            return;
        };
        let notebook_id = workspace_id.get_untracked();
        if notebook_id.is_empty() {
            return;
        }
        let next_title = title_draft.get_untracked().trim().to_string();
        if next_title.is_empty() {
            set_title_draft.set(workspace_name.get_untracked());
            set_is_editing_title.set(false);
            return;
        }
        let client = ApiClient::new(api_base_url()).with_auth(token);
        spawn(async move {
            let current_description = client
                .get_notebook(&notebook_id)
                .await
                .map(|response| response.notebook.description)
                .unwrap_or_default();
            match client
                .update_notebook(
                    &notebook_id,
                    &UpdateNotebookRequest {
                        name: next_title.clone(),
                        description: current_description,
                    },
                )
                .await
            {
                Ok(response) => {
                    set_workspace_name.set(response.notebook.title);
                    set_is_editing_title.set(false);
                }
                Err(error) => {
                    set_workspace_error.set(format!(
                        "{}: {}",
                        choose(locale.get_untracked(), "更新工作台标题失败", "Failed to rename notebook"),
                        error
                    ));
                }
            }
        });
    };

    let auth_for_create = auth.clone();
    let navigate_for_create = navigate.clone();
    let create_notebook = Arc::new(move || {
        let Some(token) = auth_for_create.token.get_untracked() else {
            return;
        };
        let name = new_notebook_name.get_untracked().trim().to_string();
        if name.is_empty() {
            return;
        }
        set_creating_notebook.set(true);
        let client = ApiClient::new(api_base_url()).with_auth(token);
        let navigate = navigate_for_create.clone();
        spawn(async move {
            match client
                .create_notebook(&CreateNotebookRequest {
                    name: name.clone(),
                    description: String::new(),
                })
                .await
            {
                Ok(response) => {
                    set_show_create_modal.set(false);
                    set_new_notebook_name.set(String::new());
                    set_creating_notebook.set(false);
                    navigate(
                        &format!("/dashboard/{}", response.notebook.id),
                        leptos_router::NavigateOptions::default(),
                    );
                }
                Err(error) => {
                    set_creating_notebook.set(false);
                    set_workspace_error.set(format!(
                        "{}: {}",
                        choose(locale.get_untracked(), "创建知识库失败", "Failed to create notebook"),
                        error
                    ));
                }
            }
        });
    });
    let create_notebook = StoredValue::new(create_notebook as Arc<dyn Fn() + Send + Sync>);
    let auth_for_logout = StoredValue::new(auth.clone());
    let navigate_to_login = StoredValue::new(navigate.clone());

    view! {
        <div class="z-20 flex items-center gap-2 border-b border-border bg-card/90 px-3 py-3 shadow-sm backdrop-blur md:gap-3 md:px-5 md:py-4">
            <A href="/dashboard" attr:class="app-button-ghost -ml-1 flex shrink-0 items-center gap-2 px-2 md:px-3">
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"/>
                </svg>
                <span class="hidden sm:inline">{move || choose(locale.get(), "返回", "Back")}</span>
            </A>

            <div class="min-w-0 flex-1">
                <p class="text-[10px] font-medium text-muted-foreground sm:text-[11px]">
                    {"Context-OS"}
                </p>
                <Show
                    when=move || is_editing_title.get()
                    fallback=move || view! {
                        <button
                            type="button"
                            class="truncate text-left text-base font-semibold text-card-foreground md:text-lg"
                            on:click=move |_| {
                                set_title_draft.set(workspace_name.get_untracked());
                                set_is_editing_title.set(true);
                            }
                        >
                            {move || {
                                let title = workspace_name.get();
                                if title.is_empty() {
                                    workspace_id.get()
                                } else {
                                    title
                                }
                            }}
                        </button>
                    }
                >
                    <input
                        type="text"
                        class="app-input h-10 max-w-md"
                        prop:value=move || title_draft.get()
                        on:input=move |ev| set_title_draft.set(event_target_value(&ev))
                        on:blur=move |_| save_workspace_title()
                        on:keydown=move |ev| {
                            let key = ev.key();
                            if key == "Enter" {
                                save_workspace_title();
                            } else if key == "Escape" {
                                set_title_draft.set(workspace_name.get_untracked());
                                set_is_editing_title.set(false);
                            }
                        }
                    />
                </Show>
            </div>

            <div class="ml-auto flex shrink-0 items-center gap-1.5 md:gap-2">
                <button
                    class="shrink-0 rounded-lg p-2 text-muted-foreground hover:bg-muted hover:text-foreground md:hidden"
                    title={move || choose(locale.get(), "线程", "Threads")}
                    on:click=move |_| set_left_rail_open.set(true)
                >
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
                    </svg>
                </button>
                <button
                    class="shrink-0 rounded-lg p-2 text-muted-foreground hover:bg-muted hover:text-foreground md:hidden"
                    title={move || choose(locale.get(), "资料与笔记", "Sources & Notes")}
                    on:click=move |_| set_right_rail_open.set(true)
                >
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7"/>
                    </svg>
                </button>

                <button
                    type="button"
                    class="app-button-secondary hidden md:inline-flex"
                    on:click=move |_| set_show_create_modal.set(true)
                >
                    {move || choose(locale.get(), "新建知识库", "New Notebook")}
                </button>
                <A
                    href={format!("/dashboard/{}/analysis", workspace_id.get_untracked())}
                    attr:class="app-button-secondary hidden md:inline-flex"
                >
                    {move || choose(locale.get(), "分析", "Analyze")}
                </A>
                <Show when=move || ui_capabilities().shared_kb>
                    <A
                        href={format!("/dashboard/{}/share", workspace_id.get_untracked())}
                        attr:class="app-button-secondary hidden md:inline-flex"
                    >
                        {move || choose(locale.get(), "分享", "Share")}
                    </A>
                </Show>
                <A
                    href={format!("/dashboard/{}/api-access", workspace_id.get_untracked())}
                    attr:class="app-button-secondary hidden md:inline-flex"
                >
                    {move || choose(locale.get(), "API", "API")}
                </A>
                <A href="/settings" attr:class="app-button-secondary hidden md:inline-flex">
                    {move || choose(locale.get(), "设置", "Settings")}
                </A>

                <div class="relative hidden md:block">
                    <button
                        type="button"
                        class="app-button-secondary"
                        on:click=move |_| set_user_menu_open.update(|open| *open = !*open)
                    >
                        {move || choose(locale.get(), "用户", "User")}
                    </button>
                    <Show when=move || user_menu_open.get()>
                        <div class="absolute right-0 top-12 z-30 w-44 rounded-2xl border border-border bg-card p-2 shadow-lg">
                            <A
                                href="/settings"
                                attr:class="block rounded-xl px-3 py-2 text-sm text-foreground hover:bg-muted"
                            >
                                {move || choose(locale.get(), "Profile", "Profile")}
                            </A>
                            <A
                                href="/settings"
                                attr:class="block rounded-xl px-3 py-2 text-sm text-foreground hover:bg-muted"
                            >
                                {move || choose(locale.get(), "Notifications", "Notifications")}
                            </A>
                            <button
                                type="button"
                                class="block w-full rounded-xl px-3 py-2 text-left text-sm text-red-600 hover:bg-red-50"
                                on:click=move |_| {
                                    auth_for_logout.with_value(|state| state.logout());
                                    navigate_to_login.with_value(|go| {
                                        go("/login", leptos_router::NavigateOptions::default())
                                    });
                                }
                            >
                                {move || choose(locale.get(), "退出登录", "Logout")}
                            </button>
                        </div>
                    </Show>
                </div>
                <LocaleToggle />
            </div>

            <Show when=move || show_create_modal.get()>
                <div
                    class="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4"
                    on:click=move |_| set_show_create_modal.set(false)
                >
                    <div
                        class="w-full max-w-md rounded-[28px] border border-border bg-card p-5 shadow-xl"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        <div class="text-lg font-semibold text-foreground">
                            {move || choose(locale.get(), "创建新知识库", "Create a notebook")}
                        </div>
                        <p class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "输入名称后立即进入新的 workspace。", "Name it and jump straight into the new workspace.")}
                        </p>
                        <input
                            type="text"
                            class="app-input mt-4"
                            placeholder={move || choose(locale.get(), "例如：市场研究 / 论文笔记", "For example: Market research / Paper notes")}
                            prop:value=move || new_notebook_name.get()
                            on:input=move |ev| set_new_notebook_name.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    create_notebook.with_value(|callback| callback());
                                }
                            }
                        />
                        <div class="mt-4 flex items-center justify-end gap-2">
                            <button
                                type="button"
                                class="app-button-ghost"
                                on:click=move |_| set_show_create_modal.set(false)
                            >
                                {move || choose(locale.get(), "取消", "Cancel")}
                            </button>
                            <button
                                type="button"
                                class="app-button-primary"
                                disabled=move || new_notebook_name.get().trim().is_empty() || creating_notebook.get()
                                on:click=move |_| create_notebook.with_value(|callback| callback())
                            >
                                {move || {
                                    if creating_notebook.get() {
                                        choose(locale.get(), "创建中...", "Creating...")
                                    } else {
                                        choose(locale.get(), "创建并进入", "Create and open")
                                    }
                                }}
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
