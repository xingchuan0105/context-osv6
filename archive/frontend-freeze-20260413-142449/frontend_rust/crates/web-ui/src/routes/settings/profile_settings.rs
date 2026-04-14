#[component]
fn ProfileSettings() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    if !ui_capabilities().profile_edit {
        return view! {
            <div class="space-y-6">
                <UsageLimitCard />
                <UnavailableFeatureCard
                    title={choose(locale.get_untracked(), "个人资料", "Profile").to_string()}
                    description={choose(
                        locale.get_untracked(),
                        "当前版本暂不开放资料编辑，请稍后再试。",
                        "Profile editing is not available in this build yet."
                    ).to_string()}
                />
            </div>
        }
        .into_any();
    }

    let auth_for_view = auth.clone();
    let (full_name, set_full_name) = signal(
        auth.user
            .get_untracked()
            .map(|user| user.full_name)
            .unwrap_or_default(),
    );
    let (saving, set_saving) = signal(false);
    let (message, set_message) = signal(String::new());
    let (error, set_error) = signal(String::new());

    let handle_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(token) = auth.token.get() else {
            set_error
                .set(choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string());
            return;
        };

        set_saving.set(true);
        set_message.set(String::new());
        set_error.set(String::new());
        let full_name_value = full_name.get().trim().to_string();
        let auth = auth.clone();

        spawn(async move {
            let client = ApiClient::new(api_base_url()).with_auth(token.clone());
            match client.update_profile(Some(full_name_value)).await {
                Ok(resp) if resp.success => {
                    if let Some(data) = resp.data {
                        auth.set_auth(token, data.user);
                    }
                    set_message.set(
                        choose(locale.get_untracked(), "资料已更新", "Profile updated").to_string(),
                    );
                }
                Ok(resp) => {
                    set_error.set(resp.error.unwrap_or_else(|| {
                        choose(
                            locale.get_untracked(),
                            "更新资料失败",
                            "Failed to update profile",
                        )
                        .to_string()
                    }));
                }
                Err(error) => {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "更新资料失败",
                            "Failed to update profile"
                        ),
                        error
                    ));
                }
            }
            set_saving.set(false);
        });
    };

    view! {
        <div class="space-y-6">
            <UsageLimitCard />
            <div class="app-surface-card">
                <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                    {move || choose(locale.get(), "个人资料", "Profile")}
                </h3>
            <form on:submit=handle_submit class="space-y-4">
                <div>
                    <label class="app-form-label">
                        {move || choose(locale.get(), "邮箱", "Email")}
                    </label>
                    <input
                        type="email"
                        class="app-input bg-muted/40 text-muted-foreground"
                        value=move || auth_for_view.user.get().map(|user| user.email).unwrap_or_default()
                        readonly
                    />
                </div>
                <div>
                    <label class="app-form-label">
                        {move || choose(locale.get(), "姓名", "Full Name")}
                    </label>
                    <input
                        type="text"
                        class="app-input"
                        value=move || full_name.get()
                        on:input=move |ev| set_full_name.set(event_target_value(&ev))
                    />
                </div>

                <Show when=move || !message.get().is_empty()>
                    <div class="rounded border border-green-200 bg-green-50 px-3 py-2 text-sm text-green-700">{message.get()}</div>
                </Show>
                <Show when=move || !error.get().is_empty()>
                    <ErrorBanner message={error.get()} />
                </Show>

                <button
                    type="submit"
                    class="app-button-primary"
                    disabled=move || saving.get()
                >
                    {move || {
                        if saving.get() {
                            choose(locale.get(), "保存中...", "Saving...")
                        } else {
                            choose(locale.get(), "保存资料", "Save Profile")
                        }
                    }}
                </button>
            </form>
            </div>
        </div>
    }
    .into_any()
}
