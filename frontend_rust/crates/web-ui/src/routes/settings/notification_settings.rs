#[component]
fn NotificationSettings() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (notifications, set_notifications) = signal(Vec::<NotificationRow>::new());
    let (preferences, set_preferences) = signal(NotificationPreferences::default());
    let (loading, set_loading) = signal(false);
    let (preferences_loading, set_preferences_loading) = signal(false);
    let (preferences_saving, set_preferences_saving) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());
    let (pending_mark_read, set_pending_mark_read) = signal(Option::<String>::None);
    let (processing_mark_read, set_processing_mark_read) = signal(String::new());

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || auth_for_load.token.get().unwrap_or_default(),
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            set_loading.set(true);
            set_preferences_loading.set(true);
            set_error.set(String::new());
            let client = ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.get_user_preferences().await {
                    Ok(user_preferences) => set_preferences.set(user_preferences.notifications),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载通知偏好失败",
                            "Failed to load notification preferences"
                        ),
                        error
                    )),
                }
                set_preferences_loading.set(false);
                match client.list_notifications().await {
                    Ok(resp) => set_notifications.set(resp.notifications),
                    Err(error) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载通知失败",
                            "Failed to load notifications"
                        ),
                        error
                    )),
                }
                set_loading.set(false);
            });
        },
    );

    let save_preferences = {
        let auth = auth.clone();
        move || {
            let Some(token) = auth.token.get() else {
                return;
            };
            set_preferences_saving.set(true);
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let next_preferences = preferences.get();
            spawn(async move {
                let result = async {
                    let mut user_preferences: UserPreferences =
                        client.get_user_preferences().await?;
                    user_preferences.notifications = next_preferences;
                    client.update_user_preferences(&user_preferences).await
                }
                .await;

                if let Err(error) = result {
                    set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "保存通知偏好失败",
                            "Failed to save notification preferences"
                        ),
                        error
                    ));
                }
                set_preferences_saving.set(false);
            });
        }
    };

    Effect::new(move |_| {
        let Some(notification_id) = pending_mark_read.get() else {
            return;
        };
        if notification_id.is_empty() || processing_mark_read.get() == notification_id {
            return;
        }
        set_processing_mark_read.set(notification_id.clone());
        let auth = auth.clone();
        spawn(async move {
            if let Some(token) = auth.token.get_untracked() {
                let client = ApiClient::new(api_base_url()).with_auth(token);
                match client.mark_notification_read(&notification_id).await {
                    Ok(_) => {
                        set_notifications.update(|items| {
                            for item in items.iter_mut() {
                                if item.id == notification_id {
                                    item.read_at = Some("read".to_string());
                                }
                            }
                        });
                    }
                    Err(error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "标记通知已读失败",
                                "Failed to mark notification as read"
                            ),
                            error
                        ));
                        set_pending_mark_read.set(None);
                        set_processing_mark_read.set(String::new());
                        return;
                    }
                }
            }
            set_pending_mark_read.set(None);
            set_processing_mark_read.set(String::new());
        });
    });

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "通知", "Notifications")}
            </h3>
            <div class="app-inline-surface mb-6">
                <div class="flex items-start justify-between gap-4">
                    <div>
                        <div class="text-sm font-semibold text-card-foreground">
                            {move || choose(locale.get(), "通知偏好", "Notification preferences")}
                        </div>
                        <div class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "控制产品更新、安全提醒和摘要通知。", "Control product updates, security alerts, and digest notifications.")}
                        </div>
                    </div>
                    <button
                        type="button"
                        class="app-button-secondary px-3 py-1.5"
                        disabled=move || preferences_loading.get() || preferences_saving.get()
                        on:click=move |_| save_preferences()
                    >
                        {move || {
                            if preferences_saving.get() {
                                choose(locale.get(), "保存中...", "Saving...")
                            } else {
                                choose(locale.get(), "保存偏好", "Save preferences")
                            }
                        }}
                    </button>
                </div>
                <div class="mt-4 grid gap-3 md:grid-cols-2">
                    <label class="app-toggle-row text-foreground">
                        <span>{move || choose(locale.get(), "邮件提醒", "Email alerts")}</span>
                        <input
                            type="checkbox"
                            checked=move || preferences.get().email_enabled
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_preferences.update(|prefs| prefs.email_enabled = checked);
                            }
                        />
                    </label>
                    <label class="app-toggle-row text-foreground">
                        <span>{move || choose(locale.get(), "产品动态", "Product updates")}</span>
                        <input
                            type="checkbox"
                            checked=move || preferences.get().product_enabled
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_preferences.update(|prefs| prefs.product_enabled = checked);
                            }
                        />
                    </label>
                    <label class="app-toggle-row text-foreground">
                        <span>{move || choose(locale.get(), "安全提醒", "Security alerts")}</span>
                        <input
                            type="checkbox"
                            checked=move || preferences.get().security_enabled
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_preferences.update(|prefs| prefs.security_enabled = checked);
                            }
                        />
                    </label>
                    <label class="app-toggle-row text-foreground">
                        <span>{move || choose(locale.get(), "每周摘要", "Weekly digest")}</span>
                        <input
                            type="checkbox"
                            checked=move || preferences.get().weekly_digest_enabled
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                set_preferences.update(|prefs| prefs.weekly_digest_enabled = checked);
                            }
                        />
                    </label>
                </div>
                <div class="mt-3 grid gap-3 md:grid-cols-2">
                    <input
                        type="text"
                        class="app-input"
                        placeholder={move || choose(locale.get(), "静默开始时间，例如 22:00", "Quiet hours start, e.g. 22:00")}
                        value=move || preferences.get().quiet_hours_start.unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            set_preferences.update(|prefs| {
                                prefs.quiet_hours_start = (!value.trim().is_empty()).then_some(value);
                            });
                        }
                    />
                    <input
                        type="text"
                        class="app-input"
                        placeholder={move || choose(locale.get(), "静默结束时间，例如 08:00", "Quiet hours end, e.g. 08:00")}
                        value=move || preferences.get().quiet_hours_end.unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            set_preferences.update(|prefs| {
                                prefs.quiet_hours_end = (!value.trim().is_empty()).then_some(value);
                            });
                        }
                    />
                </div>
            </div>
            <Show when=move || loading.get()>
                <LoadingMessage
                    message={choose(locale.get(), "正在加载通知...", "Loading notifications...").to_string()}
                />
            </Show>
            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>
            <Show when=move || !loading.get() && notifications.get().is_empty() && error.get().is_empty()>
                <EmptyMessage
                    message={choose(locale.get(), "暂时没有通知", "No notifications yet").to_string()}
                />
            </Show>
            <div class="space-y-3">
                {move || notifications.get().into_iter().map(|item| {
                    let set_pending_mark_read = set_pending_mark_read;
                    view! {
                        <NotificationCard
                            item=item
                            set_pending_mark_read=set_pending_mark_read
                        />
                    }
                }).collect_view()}
            </div>
        </div>
    }
}
