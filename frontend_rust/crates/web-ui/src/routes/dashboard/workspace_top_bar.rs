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
    let location = leptos_router::hooks::use_location();
    let is_preview_route =
        Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let is_preview_for_dashboard = is_preview_route.clone();
    let dashboard_href = Memo::new(move |_| {
        if is_preview_for_dashboard.get() {
            "/preview/live/dashboard".to_string()
        } else {
            "/dashboard".to_string()
        }
    });
    let is_preview_for_search = is_preview_route.clone();
    let search_href = Memo::new(move |_| {
        if is_preview_for_search.get() {
            "/preview/live/dashboard".to_string()
        } else {
            "/dashboard/search".to_string()
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
    let is_preview_for_help = is_preview_route.clone();
    let help_href = Memo::new(move |_| {
        if is_preview_for_help.get() {
            "/preview/live/help".to_string()
        } else {
            "/help".to_string()
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
    let (is_editing_title, set_is_editing_title) = signal(false);
    let (title_draft, set_title_draft) = signal(String::new());
    let (show_create_modal, set_show_create_modal) = signal(false);
    let (new_notebook_name, set_new_notebook_name) = signal(String::new());
    let (share_panel_open, set_share_panel_open) = signal(false);
    let (settings_panel_open, set_settings_panel_open) = signal(false);
    let (share_feedback, set_share_feedback) = signal(String::new());
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
                        choose(locale.get_untracked(), "Rename failed", "Rename failed"),
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
        let workspace_base = workspace_href_base.get_untracked();
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
                        &format!("{}/{}", workspace_base, response.notebook.id),
                        leptos_router::NavigateOptions::default(),
                    );
                }
                Err(error) => {
                    set_creating_notebook.set(false);
                    set_workspace_error.set(format!(
                        "{}: {}",
                        choose(locale.get_untracked(), "Create notebook failed", "Create notebook failed"),
                        error
                    ));
                }
            }
        });
    });
    let create_notebook = StoredValue::new(create_notebook as Arc<dyn Fn() + Send + Sync>);

    let close_quick_panels = move || {
        set_share_panel_open.set(false);
        set_settings_panel_open.set(false);
    };

    view! {
        <div class="workspace-topbar z-20">
            <Show when=move || share_panel_open.get() || settings_panel_open.get()>
                <button
                    type="button"
                    class="fixed inset-0 z-20 cursor-default bg-transparent"
                    on:click=move |_| close_quick_panels()
                />
            </Show>

                <button
                    class="inline-flex h-9 w-9 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-muted md:hidden"
                    on:click=move |_| set_left_rail_open.set(true)
                    title={move || choose(locale.get(), "Threads", "Threads")}
                >
                <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
                </svg>
            </button>

            <A href=move || dashboard_href.get() attr:class="flex shrink-0 items-center gap-2">
                <span class="inline-flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                    <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.9" d="M12 6a5 5 0 00-5 5v7h10v-7a5 5 0 00-5-5z"/>
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.9" d="M9 11h6m-4 3h2"/>
                    </svg>
                </span>
                <span class="workspace-topbar-title hidden sm:inline">{"Context-OS"}</span>
            </A>

            <span class="hidden h-7 w-px bg-border md:block"></span>

            <div class="min-w-0 flex-1">
                <Show
                    when=move || is_editing_title.get()
                    fallback=move || view! {
                        <button
                            type="button"
                            class="workspace-topbar-title truncate text-left"
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
                        class="workspace-input h-10 max-w-xl"
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

            <div class="ml-auto hidden items-center gap-1 md:flex">
                <button
                    type="button"
                    class="workspace-topbar-action workspace-topbar-action-primary"
                    on:click=move |_| set_show_create_modal.set(true)
                >
                    <span class="text-[16px] leading-none">{"+"}</span>
                    <span>{move || choose(locale.get(), "New Notebook", "New Notebook")}</span>
                </button>

                <A href=move || search_href.get() attr:class="workspace-topbar-action">
                    {move || choose(locale.get(), "检索", "Search")}
                </A>

                <Show when=move || ui_capabilities().shared_kb>
                    <div class="relative">
                        <button
                            type="button"
                            class="workspace-topbar-action"
                            on:click=move |_| {
                                set_share_panel_open.update(|open| *open = !*open);
                                set_settings_panel_open.set(false);
                                set_share_feedback.set(String::new());
                            }
                        >
                            {move || choose(locale.get(), "Share", "Share")}
                        </button>
                        <Show when=move || share_panel_open.get()>
                            <div class="workspace-menu absolute right-0 top-12 z-30 w-72 p-3 shadow-popover">
                                <div class="text-[14px] font-semibold text-foreground">
                                    {move || choose(locale.get(), "Quick Share", "Quick Share")}
                                </div>
                                <div class="mt-2 rounded-xl border border-border bg-muted px-3 py-2 text-[12px] text-muted-foreground break-all">
                                    {move || format!("{}/{}", workspace_href_base.get(), workspace_id.get())}
                                </div>
                                <div class="mt-3 flex gap-2">
                                    <button
                                        type="button"
                                        class="flex-1 rounded-xl bg-primary px-3 py-2 text-[13px] font-medium text-primary-foreground"
                                        on:click=move |_| {
                                            let share_path = format!(
                                                "{}/{}",
                                                workspace_href_base.get_untracked(),
                                                workspace_id.get_untracked()
                                            );
                                            copy_text_to_clipboard(&share_path);
                                            set_share_feedback.set(choose(locale.get_untracked(), "Link copied", "Link copied").to_string());
                                        }
                                    >
                                        {move || choose(locale.get(), "Copy Link", "Copy Link")}
                                    </button>
                                    <button
                                        type="button"
                                        class="flex-1 rounded-xl border border-border bg-card px-3 py-2 text-[13px] font-medium text-foreground"
                                        on:click=move |_| {
                                            let invite_path = format!(
                                                "{}/{}/share",
                                                workspace_href_base.get_untracked(),
                                                workspace_id.get_untracked()
                                            );
                                            copy_text_to_clipboard(&invite_path);
                                            set_share_feedback.set(choose(locale.get_untracked(), "Invite link copied", "Invite link copied").to_string());
                                        }
                                    >
                                        {move || choose(locale.get(), "Copy Invite", "Copy Invite")}
                                    </button>
                                </div>
                                <Show when=move || !share_feedback.get().is_empty()>
                                    <p class="mt-2 text-[12px] text-success">{move || share_feedback.get()}</p>
                                </Show>
                            </div>
                        </Show>
                    </div>
                </Show>

                <A
                    href=move || format!("{}/{}/api-access", workspace_href_base.get(), workspace_id.get_untracked())
                    attr:class="workspace-topbar-action"
                >
                    {"API"}
                </A>

                <div class="relative">
                    <button
                        type="button"
                        class="inline-flex h-9 w-9 items-center justify-center rounded-full border border-border bg-card text-muted-foreground transition-colors hover:bg-muted"
                        on:click=move |_| {
                            set_settings_panel_open.update(|open| *open = !*open);
                            set_share_panel_open.set(false);
                        }
                        title={move || choose(locale.get(), "Settings", "Settings")}
                    >
                        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M10.325 4.317a1 1 0 011.35-.936l1.09.445a1 1 0 00.77 0l1.09-.445a1 1 0 011.35.936l.11 1.167a1 1 0 00.55.79l1.004.58a1 1 0 01.365 1.366l-.566.98a1 1 0 000 .99l.566.98a1 1 0 01-.365 1.366l-1.005.58a1 1 0 00-.55.79l-.109 1.167a1 1 0 01-1.35.936l-1.09-.445a1 1 0 00-.77 0l-1.09.445a1 1 0 01-1.35-.936l-.11-1.167a1 1 0 00-.55-.79l-1.004-.58a1 1 0 01-.365-1.366l.566-.98a1 1 0 000-.99l-.566-.98a1 1 0 01.365-1.366l1.004-.58a1 1 0 00.55-.79l.11-1.167z"/>
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M12 15a3 3 0 100-6 3 3 0 000 6z"/>
                        </svg>
                    </button>
                    <Show when=move || settings_panel_open.get()>
                        <div class="workspace-menu absolute right-0 top-12 z-30 w-56 p-3 shadow-popover">
                            <A href=move || settings_appearance_href.get() attr:class="workspace-menu-item">
                                {move || choose(locale.get(), "Open Settings", "Open Settings")}
                            </A>
                            <A href=move || help_href.get() attr:class="workspace-menu-item mt-1">
                                {move || choose(locale.get(), "Help", "Help")}
                            </A>
                        </div>
                    </Show>
                </div>

                <A
                    href=move || settings_profile_href.get()
                    attr:class="inline-flex h-9 w-9 items-center justify-center rounded-full border border-border bg-card text-muted-foreground transition-colors hover:bg-muted"
                    attr:title={move || choose(locale.get(), "Account", "Account")}
                >
                    <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 12a4 4 0 100-8 4 4 0 000 8z"/>
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 20a8 8 0 0116 0"/>
                    </svg>
                </A>
            </div>

            <button
                class="inline-flex h-9 w-9 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-muted md:hidden"
                on:click=move |_| set_right_rail_open.set(true)
                title={move || choose(locale.get(), "Sources & Notes", "Sources & Notes")}
            >
                <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7"/>
                </svg>
            </button>

            <Show when=move || show_create_modal.get()>
                <div class="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4" on:click=move |_| set_show_create_modal.set(false)>
                    <div class="w-full max-w-md rounded-[28px] border border-border bg-card p-5 shadow-xl" on:click=move |ev| ev.stop_propagation()>
                        <div class="text-lg font-semibold text-foreground">
                            {move || choose(locale.get(), "Create a notebook", "Create a notebook")}
                        </div>
                        <p class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "Name it and open immediately.", "Name it and open immediately.")}
                        </p>
                        <input
                            type="text"
                            class="workspace-input mt-4 h-11"
                            placeholder={move || choose(locale.get(), "e.g. Project research", "e.g. Project research")}
                            prop:value=move || new_notebook_name.get()
                            on:input=move |ev| set_new_notebook_name.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    create_notebook.with_value(|callback| callback());
                                }
                            }
                        />
                        <div class="mt-4 flex items-center justify-end gap-2">
                            <button type="button" class="rounded-xl px-3 py-2 text-sm text-foreground hover:bg-muted" on:click=move |_| set_show_create_modal.set(false)>
                                {move || choose(locale.get(), "Cancel", "Cancel")}
                            </button>
                            <button
                                type="button"
                                class="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                                disabled=move || new_notebook_name.get().trim().is_empty() || creating_notebook.get()
                                on:click=move |_| create_notebook.with_value(|callback| callback())
                            >
                                {move || if creating_notebook.get() { choose(locale.get(), "Creating...", "Creating...") } else { choose(locale.get(), "Create", "Create") }}
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
