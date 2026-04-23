#[component]
fn SecuritySettings() -> impl IntoView {
    let auth = use_auth_state();
    let ui_prefs = use_ui_prefs_state();
    let locale = ui_prefs.locale;
    let theme = ui_prefs.theme;
    let password_reset_enabled = use_password_reset_enabled();
    let navigate = use_navigate();
    let location = use_location();
    let location_for_login = location.clone();
    let login_path =
        Memo::new(move |_| scoped_settings_auth_path(&location_for_login.pathname.get(), "/login"));
    let location_for_reset = location.clone();
    let reset_password_path =
        Memo::new(move |_| {
            scoped_settings_auth_path(&location_for_reset.pathname.get(), "/reset-password")
        });
    let auth_for_password_change = auth.clone();
    let navigate_for_password_change = navigate.clone();
    let login_path_for_password_change = login_path.clone();
    let (current_password, set_current_password) = signal(String::new());
    let (new_password, set_new_password) = signal(String::new());
    let (confirm_password, set_confirm_password) = signal(String::new());
    let (saving, set_saving) = signal(false);
    let (message, set_message) = signal(String::new());
    let (error, set_error) = signal(String::new());

    let handle_password_change = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(token) = auth.token.get() else {
            set_error
                .set(choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string());
            return;
        };

        if new_password.get() != confirm_password.get() {
            set_error.set(
                choose(
                    locale.get_untracked(),
                    "两次输入的密码不一致",
                    "Passwords do not match",
                )
                .to_string(),
            );
            return;
        }

        set_saving.set(true);
        set_message.set(String::new());
        set_error.set(String::new());

        let req = ChangePasswordRequest {
            old_password: current_password.get(),
            new_password: new_password.get(),
        };
        let auth_for_async = auth_for_password_change.clone();
        let navigate_for_async = navigate_for_password_change.clone();
        let login_path_for_async = login_path_for_password_change.get_untracked();

        spawn(async move {
            let client = ApiClient::new(api_base_url()).with_auth(token);
            match client.change_password(&req).await {
                Ok(resp) if resp.success => {
                    set_current_password.set(String::new());
                    set_new_password.set(String::new());
                    set_confirm_password.set(String::new());
                    auth_for_async.logout();
                    navigate_for_async(&login_path_for_async, NavigateOptions::default());
                }
                Ok(resp) => {
                    set_error.set(resp.error.unwrap_or_else(|| {
                        choose(
                            locale.get_untracked(),
                            "修改密码失败",
                            "Failed to change password",
                        )
                        .to_string()
                    }));
                }
                Err(error) => {
                    set_error.set(describe_auth_error(
                        locale.get_untracked(),
                        &choose(
                            locale.get_untracked(),
                            "修改密码失败",
                            "Failed to change password",
                        )
                        .to_string(),
                        &error,
                    ));
                }
            }
            set_saving.set(false);
        });
    };

    let auth_for_logout = auth.clone();
    let navigate_for_logout = navigate.clone();
    let handle_logout = move |_| {
        let token = auth_for_logout.token.get_untracked();
        let auth = auth_for_logout.clone();
        let navigate = navigate_for_logout.clone();
        let logout_path = login_path.get_untracked();
        spawn(async move {
            logout_current_session(token).await;
            auth.logout();
            navigate(&logout_path, NavigateOptions::default());
        });
    };

    view! {
        <div class="space-y-6">
            <div class="app-surface-card">
                <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || choose(locale.get(), "修改密码", "Change Password")}
                </h3>
                <form on:submit=handle_password_change class="space-y-4">
                    <input
                        type="password"
                        class="app-input"
                        placeholder={move || choose(locale.get(), "当前密码", "Current password")}
                        value=move || current_password.get()
                        on:input=move |ev| set_current_password.set(event_target_value(&ev))
                    />
                    <input
                        type="password"
                        class="app-input"
                        placeholder={move || choose(locale.get(), "新密码", "New password")}
                        value=move || new_password.get()
                        on:input=move |ev| set_new_password.set(event_target_value(&ev))
                    />
                    <input
                        type="password"
                        class="app-input"
                        placeholder={move || choose(locale.get(), "确认新密码", "Confirm new password")}
                        value=move || confirm_password.get()
                        on:input=move |ev| set_confirm_password.set(event_target_value(&ev))
                    />

                    <Show when=move || !message.get().is_empty()>
                        <div class="rounded border border-green-200 bg-green-50 px-3 py-2 text-sm text-green-700">{message.get()}</div>
                    </Show>
                    <Show when=move || !error.get().is_empty()>
                        <ErrorBanner message={error.get()} />
                    </Show>

                    <button type="submit" class="app-button-primary" disabled=move || saving.get()>
                        {move || if saving.get() {
                            choose(locale.get(), "更新中...", "Updating...")
                        } else {
                            choose(locale.get(), "更新密码", "Update Password")
                        }}
                    </button>
                </form>
            </div>

            <Show when=move || password_reset_enabled.get()>
                <div class="mt-6 rounded-xl border border-border bg-card px-4 py-4">
                    <div class="flex items-center justify-between">
                        <div>
                            <h3 class="text-sm font-medium text-card-foreground">
                                {move || choose(locale.get(), "重置密码", "Reset Password")}
                            </h3>
                            <p class="mt-1 text-xs text-muted-foreground">
                                {move || choose(locale.get(),
                                    "通过邮箱验证码重置密码",
                                    "Reset your password with an email verification code")}
                            </p>
                        </div>
                        <A href=move || reset_password_path.get() attr:class="app-button-secondary">
                            {move || choose(locale.get(), "重置密码", "Reset Password")}
                        </A>
                    </div>
                </div>
            </Show>

            <div class="app-surface-card">
                <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || choose(locale.get(), "当前会话", "Session")}
                </h3>
                <div class="space-y-3 text-sm text-muted-foreground">
                    <div class="app-inline-surface">
                        <div class="font-medium text-card-foreground">
                            {move || current_device_info(locale.get()).label}
                        </div>
                        <div class="mt-1">
                            {move || {
                                format!(
                                    "{} {} · {} {} · {} {}",
                                    choose(locale.get(), "时区", "Timezone"),
                                    current_device_info(locale.get()).timezone,
                                    choose(locale.get(), "界面语言", "Language"),
                                    match locale.get() {
                                        Locale::ZhCn => "简体中文",
                                        Locale::En => "English",
                                    },
                                    choose(locale.get(), "主题", "Theme"),
                                    match theme.get() {
                                        Theme::System => choose(locale.get(), "跟随系统", "System"),
                                        Theme::Light => choose(locale.get(), "浅色", "Light"),
                                        Theme::Dark => choose(locale.get(), "深色", "Dark"),
                                    }
                                )
                            }}
                        </div>
                    </div>
                    <p>
                        {move || choose(locale.get(), "退出当前设备上的登录状态。", "Sign out on this device.")}
                    </p>
                    <p class="text-xs text-muted-foreground">
                        {move || {
                            auth.user
                                .get()
                                .map(|user| format!(
                                    "{} {}",
                                    choose(locale.get(), "当前账号：", "Signed in as"),
                                    user.email
                                ))
                                .unwrap_or_default()
                        }}
                    </p>
                </div>
                <button class="app-button-secondary" on:click=handle_logout>
                    {move || choose(locale.get(), "退出登录", "Sign Out")}
                </button>
            </div>
        </div>
    }
}
