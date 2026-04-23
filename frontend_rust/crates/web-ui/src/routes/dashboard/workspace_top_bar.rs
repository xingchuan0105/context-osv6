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
    let ui_prefs = use_ui_prefs_state();
    let navigate = leptos_router::hooks::use_navigate();
    let location = leptos_router::hooks::use_location();
    let is_preview_route = Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));
    let is_preview_for_dashboard = is_preview_route.clone();
    let dashboard_href = Memo::new(move |_| {
        if is_preview_for_dashboard.get() {
            "/preview/live/dashboard".to_string()
        } else {
            "/dashboard".to_string()
        }
    });
    let is_preview_for_profile = is_preview_route.clone();
    let profile_href = Memo::new(move |_| {
        if is_preview_for_profile.get() {
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
    let is_preview_for_share = is_preview_route.clone();
    let share_href = Memo::new(move |_| {
        if is_preview_for_share.get() {
            format!("{}/{}/share", workspace_href_base.get(), workspace_id.get())
        } else {
            format!("{}/{}/share", workspace_href_base.get(), workspace_id.get())
        }
    });
    let is_preview_for_analyze = is_preview_route.clone();
    let analyze_href = Memo::new(move |_| {
        if is_preview_for_analyze.get() {
            format!(
                "{}/{}/analyze",
                workspace_href_base.get(),
                workspace_id.get()
            )
        } else {
            format!(
                "{}/{}/analyze",
                workspace_href_base.get(),
                workspace_id.get()
            )
        }
    });
    let (is_editing_title, set_is_editing_title) = signal(false);
    let (title_draft, set_title_draft) = signal(String::new());
    let (show_create_modal, set_show_create_modal) = signal(false);
    let (new_notebook_name, set_new_notebook_name) = signal(String::new());
    let (generated_workspace_name, set_generated_workspace_name) = signal(String::new());
    let (generated_workspace_counter_key, set_generated_workspace_counter_key) =
        signal(String::new());
    let (gear_menu_open, set_gear_menu_open) = signal(false);
    let (avatar_menu_open, set_avatar_menu_open) = signal(false);
    let (subscription, set_subscription) = signal(None::<web_sdk::dtos::SubscriptionResponse>);
    let (subscription_loaded_token, set_subscription_loaded_token) = signal(String::new());
    let (creating_notebook, set_creating_notebook) = signal(false);

    Effect::new(move |_| {
        if !is_editing_title.get() {
            set_title_draft.set(workspace_name.get());
        }
    });

    let auth_for_subscription = auth.clone();
    Effect::new(move |_| {
        let Some(token) = auth_for_subscription.token.get() else {
            if !subscription.get_untracked().is_none() {
                set_subscription.set(None);
            }
            set_subscription_loaded_token.set(String::new());
            return;
        };
        if subscription_loaded_token.get() == token {
            return;
        }
        set_subscription_loaded_token.set(token.clone());
        let client = ApiClient::new(api_base_url()).with_auth(token);
        spawn(async move {
            match client.get_subscription().await {
                Ok(resp) => set_subscription.set(Some(resp)),
                Err(_) => set_subscription.set(None),
            }
        });
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

    let locale_for_create = locale;
    let open_create_modal = move || {
        let (default_name, key) =
            workspace_default_title_for_now(locale_for_create.get_untracked());
        set_new_notebook_name.set(default_name.clone());
        set_generated_workspace_name.set(default_name);
        set_generated_workspace_counter_key.set(key);
        set_gear_menu_open.set(false);
        set_avatar_menu_open.set(false);
        set_show_create_modal.set(true);
    };

    let auth_for_create = auth.clone();
    let navigate_for_create = navigate.clone();
    let create_notebook = Arc::new(move || {
        let Some(token) = auth_for_create.token.get_untracked() else {
            return;
        };
        let generated_name = generated_workspace_name.get_untracked();
        let generated_key = generated_workspace_counter_key.get_untracked();
        let name = new_notebook_name.get_untracked().trim().to_string();
        let name = if name.is_empty() {
            generated_name.clone()
        } else {
            name
        };
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
                    set_generated_workspace_name.set(String::new());
                    set_generated_workspace_counter_key.set(String::new());
                    if !generated_name.is_empty()
                        && generated_name == name
                        && !generated_key.is_empty()
                    {
                        bump_workspace_default_title_counter(&generated_key);
                    }
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
                        choose(
                            locale.get_untracked(),
                            "Create workspace failed",
                            "Create workspace failed"
                        ),
                        error
                    ));
                }
            }
        });
    });
    let create_notebook = StoredValue::new(create_notebook as Arc<dyn Fn() + Send + Sync>);

    let close_quick_panels = move || {
        set_gear_menu_open.set(false);
        set_avatar_menu_open.set(false);
        set_show_create_modal.set(false);
    };

    let current_user_label = move || {
        auth.user
            .get()
            .and_then(|user| {
                let full_name = user.full_name.trim().to_string();
                let email = user.email.trim().to_string();
                if !full_name.is_empty() {
                    Some(full_name)
                } else if !email.is_empty() {
                    Some(email)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| choose(locale.get(), "未登录", "Not signed in").to_string())
    };
    let current_user_email = move || {
        auth.user
            .get()
            .map(|user| user.email)
            .filter(|email| !email.trim().is_empty())
            .unwrap_or_else(|| choose(locale.get(), "未登录", "Not signed in").to_string())
    };
    let current_user_tier = move || {
        subscription
            .get()
            .map(|value| {
                let plan_id = value.plan_id.to_lowercase();
                let is_vip = value.status.eq_ignore_ascii_case("active")
                    && !plan_id.contains("free")
                    && !plan_id.contains("trial");
                if is_vip {
                    choose(locale.get(), "VIP", "VIP").to_string()
                } else {
                    choose(locale.get(), "Free", "Free").to_string()
                }
            })
            .unwrap_or_else(|| choose(locale.get(), "Free", "Free").to_string())
    };
    let current_user_tier_label = move || {
        let tier = current_user_tier();
        let plan_id = subscription
            .get()
            .map(|value| value.plan_id)
            .unwrap_or_else(|| choose(locale.get(), "基础版", "Free tier").to_string());
        format!("{} · {}", tier, plan_id)
    };
    let theme_label = move |theme: crate::state::ui_prefs::Theme| match theme {
        crate::state::ui_prefs::Theme::System => {
            choose(locale.get(), "跟随系统", "System").to_string()
        }
        crate::state::ui_prefs::Theme::Light => choose(locale.get(), "浅色", "Light").to_string(),
        crate::state::ui_prefs::Theme::Dark => choose(locale.get(), "深色", "Dark").to_string(),
    };

    let open_gear_menu = move |_| {
        set_gear_menu_open.update(|open| *open = !*open);
        set_avatar_menu_open.set(false);
    };
    let open_avatar_menu = move |_| {
        set_avatar_menu_open.update(|open| *open = !*open);
        set_gear_menu_open.set(false);
    };

    let handle_logout = StoredValue::new({
        let auth = auth.clone();
        let navigate = navigate.clone();
        let is_preview_route = is_preview_route.clone();
        move || {
            let token = auth.token.get_untracked();
            let auth = auth.clone();
            set_avatar_menu_open.set(false);
            let logout_path = if is_preview_route.get_untracked() {
                "/preview/live/login"
            } else {
                "/login"
            };
            let navigate = navigate.clone();
            spawn(async move {
                logout_current_session(token).await;
                auth.logout();
                navigate(logout_path, leptos_router::NavigateOptions::default());
            });
        }
    });

    view! {
        <div class="workspace-topbar z-20">
            <Show when=move || gear_menu_open.get() || avatar_menu_open.get() || show_create_modal.get()>
                <button
                    type="button"
                    class=workspace_ui_style::topbar_backdrop
                    on:click=move |_| close_quick_panels()
                />
            </Show>

                <button
                    class=workspace_ui_style::mobile_toggle
                    on:click=move |_| set_left_rail_open.set(true)
                    title={move || choose(locale.get(), "Threads", "Threads")}
                >
                <svg class=workspace_ui_style::mobile_toggle_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
                </svg>
            </button>

            <A href=move || dashboard_href.get() attr:class=workspace_ui_style::brand_link>
                <span class=workspace_ui_style::brand_badge>
                    <ContextOsMark class=workspace_ui_style::brand_icon />
                </span>
                <span class="workspace-topbar-title hidden sm:inline">{"Context-OS"}</span>
            </A>

            <span class=workspace_ui_style::title_divider></span>

            <div class=workspace_ui_style::title_region>
                <Show
                    when=move || is_editing_title.get()
                    fallback=move || view! {
                        <button
                            type="button"
                            class={format!("workspace-topbar-title {}", workspace_ui_style::title_button)}
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
                        class={format!("workspace-input {}", workspace_ui_style::title_input)}
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

            <div class=workspace_ui_style::desktop_actions>
                <button
                    type="button"
                    class=workspace_ui_style::action_button
                    on:click=move |_| open_create_modal()
                >
                    <span class=workspace_ui_style::action_plus_icon>{"+"}</span>
                    <span>{move || choose(locale.get(), "New Workspace", "New Workspace")}</span>
                </button>

                <A
                    href=move || analyze_href.get()
                    attr:class=move || {
                        if location.pathname.get().ends_with("/analyze") {
                            format!(
                                "{} {} {}",
                                workspace_ui_style::action_button,
                                workspace_ui_style::action_link,
                                workspace_ui_style::action_button_active
                            )
                        } else {
                            format!("{} {}", workspace_ui_style::action_button, workspace_ui_style::action_link)
                        }
                    }
                >
                    {move || choose(locale.get(), "Analyze", "Analyze")}
                </A>

                <A
                    href=move || share_href.get()
                    attr:class={format!("{} {}", workspace_ui_style::action_button, workspace_ui_style::action_link)}
                >
                    {move || choose(locale.get(), "Share", "Share")}
                </A>

                <A
                    href=move || format!("{}/{}/api-access", workspace_href_base.get(), workspace_id.get_untracked())
                    attr:class={format!("{} {}", workspace_ui_style::action_button, workspace_ui_style::action_link)}
                >
                    {"API"}
                </A>

                <div class=workspace_ui_style::panel_anchor>
                    <button
                        type="button"
                        class=workspace_ui_style::round_icon_button
                        on:click=open_gear_menu
                        title={move || choose(locale.get(), "Settings", "Settings")}
                    >
                        <svg class=workspace_ui_style::round_icon_svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M10.325 4.317a1 1 0 011.35-.936l1.09.445a1 1 0 00.77 0l1.09-.445a1 1 0 011.35.936l.11 1.167a1 1 0 00.55.79l1.004.58a1 1 0 01.365 1.366l-.566.98a1 1 0 000 .99l.566.98a1 1 0 01-.365 1.366l-1.005.58a1 1 0 00-.55.79l-.109 1.167a1 1 0 01-1.35.936l-1.09-.445a1 1 0 00-.77 0l-1.09.445a1 1 0 01-1.35-.936l-.11-1.167a1 1 0 00-.55-.79l-1.004-.58a1 1 0 01-.365-1.366l.566-.98a1 1 0 000-.99l-.566-.98a1 1 0 01.365-1.366l1.004-.58a1 1 0 00.55-.79l.11-1.167z"/>
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M12 15a3 3 0 100-6 3 3 0 000 6z"/>
                        </svg>
                    </button>
                    <Show when=move || gear_menu_open.get()>
                        <div class={format!("workspace-menu {}", workspace_ui_style::menu_panel_wide)}>
                            <div class=workspace_ui_style::menu_section>
                                <div class=workspace_ui_style::menu_section_title>
                                    {move || choose(locale.get(), "主题配色", "Theme")}
                                </div>
                                <div class=workspace_ui_style::menu_choice_group>
                                    <button
                                        type="button"
                                        class=workspace_ui_style::menu_choice
                                        class=(workspace_ui_style::menu_choice_active, move || ui_prefs.theme.get() == crate::state::ui_prefs::Theme::System)
                                        on:click=move |_| {
                                            ui_prefs.set_theme.set(crate::state::ui_prefs::Theme::System);
                                            set_gear_menu_open.set(false);
                                        }
                                    >
                                        {move || theme_label(crate::state::ui_prefs::Theme::System)}
                                    </button>
                                    <button
                                        type="button"
                                        class=workspace_ui_style::menu_choice
                                        class=(workspace_ui_style::menu_choice_active, move || ui_prefs.theme.get() == crate::state::ui_prefs::Theme::Light)
                                        on:click=move |_| {
                                            ui_prefs.set_theme.set(crate::state::ui_prefs::Theme::Light);
                                            set_gear_menu_open.set(false);
                                        }
                                    >
                                        {move || theme_label(crate::state::ui_prefs::Theme::Light)}
                                    </button>
                                    <button
                                        type="button"
                                        class=workspace_ui_style::menu_choice
                                        class=(workspace_ui_style::menu_choice_active, move || ui_prefs.theme.get() == crate::state::ui_prefs::Theme::Dark)
                                        on:click=move |_| {
                                            ui_prefs.set_theme.set(crate::state::ui_prefs::Theme::Dark);
                                            set_gear_menu_open.set(false);
                                        }
                                    >
                                        {move || theme_label(crate::state::ui_prefs::Theme::Dark)}
                                    </button>
                                </div>
                            </div>

                            <div class=workspace_ui_style::menu_section>
                                <div class=workspace_ui_style::menu_section_title>
                                    {move || choose(locale.get(), "语言设置", "Language")}
                                </div>
                                <div class=workspace_ui_style::menu_choice_group>
                                    <button
                                        type="button"
                                        class=workspace_ui_style::menu_choice
                                        class=(workspace_ui_style::menu_choice_active, move || ui_prefs.locale.get() == crate::i18n::Locale::ZhCn)
                                        on:click=move |_| {
                                            ui_prefs.set_locale.set(crate::i18n::Locale::ZhCn);
                                            set_gear_menu_open.set(false);
                                        }
                                    >
                                        {move || choose(locale.get(), "中文", "Chinese")}
                                    </button>
                                    <button
                                        type="button"
                                        class=workspace_ui_style::menu_choice
                                        class=(workspace_ui_style::menu_choice_active, move || ui_prefs.locale.get() == crate::i18n::Locale::En)
                                        on:click=move |_| {
                                            ui_prefs.set_locale.set(crate::i18n::Locale::En);
                                            set_gear_menu_open.set(false);
                                        }
                                    >
                                        {move || choose(locale.get(), "English", "English")}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </Show>
                </div>

                <div class=workspace_ui_style::panel_anchor>
                    <button
                        type="button"
                        class=workspace_ui_style::round_icon_button
                        on:click=open_avatar_menu
                        title={move || choose(locale.get(), "Account", "Account")}
                    >
                        <svg class=workspace_ui_style::brand_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 12a4 4 0 100-8 4 4 0 000 8z"/>
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 20a8 8 0 0116 0"/>
                        </svg>
                    </button>
                    <Show when=move || avatar_menu_open.get()>
                        <div class={format!("workspace-menu {}", workspace_ui_style::menu_panel)}>
                            <div class=workspace_ui_style::menu_account_card>
                                <div class=workspace_ui_style::menu_account_name>
                                    {move || current_user_label()}
                                </div>
                                <div class=workspace_ui_style::menu_account_email>
                                    {move || current_user_email()}
                                </div>
                            </div>
                            <A
                                href=move || profile_href.get()
                                attr:class=workspace_ui_style::menu_choice
                                on:click=move |_| set_avatar_menu_open.set(false)
                            >
                                {move || choose(locale.get(), "账号信息", "Account information")}
                            </A>
                            <div class=workspace_ui_style::menu_section>
                                <div class=workspace_ui_style::menu_section_title>
                                    {move || choose(locale.get(), "用户标识", "User tier")}
                                </div>
                                <div class=workspace_ui_style::menu_tier_badge>
                                    {move || current_user_tier_label()}
                                </div>
                            </div>
                            <button
                                type="button"
                                class=workspace_ui_style::menu_logout_button
                                on:click=move |_| handle_logout.with_value(|logout| logout())
                            >
                                {move || choose(locale.get(), "登出", "Log out")}
                            </button>
                        </div>
                    </Show>
                </div>

                <A
                    href=move || workspace_href_base.get()
                    attr:class={format!("{} {}", workspace_ui_style::round_icon_button, workspace_ui_style::action_link)}
                    attr:title={move || choose(locale.get(), "Dashboard", "Dashboard")}
                >
                    <svg class=workspace_ui_style::brand_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 12a4 4 0 100-8 4 4 0 000 8z"/>
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 20a8 8 0 0116 0"/>
                    </svg>
                </A>
            </div>

            <button
                class=workspace_ui_style::mobile_right_toggle
                on:click=move |_| set_right_rail_open.set(true)
                title={move || choose(locale.get(), "Sources & Notes", "Sources & Notes")}
            >
                <svg class=workspace_ui_style::mobile_toggle_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7"/>
                </svg>
            </button>

            <Show when=move || show_create_modal.get()>
                <div class=workspace_ui_style::create_overlay on:click=move |_| set_show_create_modal.set(false)>
                    <div class=workspace_ui_style::create_modal on:click=move |ev| ev.stop_propagation()>
                        <div class=workspace_ui_style::create_title>
                            {move || choose(locale.get(), "Create a workspace", "Create a workspace")}
                        </div>
                        <p class=workspace_ui_style::create_desc>
                            {move || choose(locale.get(), "Name it and open immediately.", "Name it and open immediately.")}
                        </p>
                        <input
                            type="text"
                            class={format!("workspace-input {}", workspace_ui_style::create_input)}
                            placeholder={move || choose(locale.get(), "e.g. Project research", "e.g. Project research")}
                            prop:value=move || new_notebook_name.get()
                            on:input=move |ev| set_new_notebook_name.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    create_notebook.with_value(|callback| callback());
                                }
                            }
                        />
                        <div class=workspace_ui_style::create_actions>
                            <button type="button" class=workspace_ui_style::create_cancel on:click=move |_| set_show_create_modal.set(false)>
                                {move || choose(locale.get(), "Cancel", "Cancel")}
                            </button>
                            <button
                                type="button"
                                class=workspace_ui_style::create_confirm
                                disabled=move || creating_notebook.get()
                                on:click=move |_| create_notebook.with_value(|callback| callback())
                            >
                                {move || if creating_notebook.get() { choose(locale.get(), "Creating...", "Creating...") } else { choose(locale.get(), "Create Workspace", "Create Workspace") }}
                            </button>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
