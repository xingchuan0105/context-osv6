#[component]
fn FeatureFlagCard(
    flag: FeatureFlagEntry,
    locale: ReadSignal<Locale>,
    auth: crate::state::auth::AuthState,
    busy_action: ReadSignal<String>,
    set_busy_action: WriteSignal<String>,
    set_error: WriteSignal<String>,
    set_reload_nonce: WriteSignal<u64>,
) -> impl IntoView {
    let flag_key = flag.key.clone();
    let flag_key_for_display = flag.key.clone();
    let flag_key_for_override_prompt = StoredValue::new(flag.key.clone());
    let flag_key_for_override_match = StoredValue::new(flag.key.clone());
    let flag_key_for_override_execute = StoredValue::new(flag.key.clone());
    let flag_enabled = flag.enabled;
    let flag_effective_enabled = flag.effective_enabled;
    let flag_requires_config = flag.requires_config;
    let flag_config_ready = flag.config_ready;
    let flag_has_pending_request = flag.has_pending_request;
    let (reason, set_reason) = signal(String::new());
    let (show_override_confirm, set_show_override_confirm) = signal(false);
    let (override_phrase, set_override_phrase) = signal(String::new());
    let request_action_key = StoredValue::new(format!("request:{}", flag_key));
    let request_action_key_for_disabled = request_action_key;
    let override_action_key = StoredValue::new(format!("override:{}", flag.key));
    let override_action_key_for_disabled = override_action_key;

    view! {
        <div class="rounded-lg border border-border px-4 py-4">
            <div class="flex items-start justify-between gap-4">
                <div>
                    <div class="font-medium text-foreground">{flag_key_for_display}</div>
                    <div class="mt-1 text-sm text-muted-foreground">{flag.description.clone()}</div>
                    <div class="mt-2 flex flex-wrap gap-2 text-xs">
                        <span class="rounded-full bg-slate-100 px-2 py-1 text-slate-700">
                            {feature_flag_category_label(locale.get(), &flag.category)}
                        </span>
                        <span class="rounded-full bg-sky-100 px-2 py-1 text-sky-800">
                            {move || choose(locale.get(), "来源：", "Source: ")}
                            {feature_flag_source_label(locale.get(), &flag.source)}
                        </span>
                    </div>
                    <div class="mt-2 flex flex-wrap gap-2 text-xs">
                        <span class="rounded-full bg-muted px-2 py-1">
                            {move || choose(locale.get(), "期望：", "desired: ")}
                            {feature_flag_toggle_label(locale.get(), flag_enabled)}
                        </span>
                        <span class="rounded-full bg-muted px-2 py-1">
                            {move || choose(locale.get(), "生效：", "effective: ")}
                            {feature_flag_toggle_label(locale.get(), flag_effective_enabled)}
                        </span>
                        <span class="rounded-full bg-muted px-2 py-1">
                            {move || choose(locale.get(), "配置：", "config: ")}
                            {feature_flag_config_label(locale.get(), flag_config_ready)}
                        </span>
                        <Show when=move || flag_requires_config>
                            <span class="rounded-full bg-violet-100 px-2 py-1 text-violet-800">
                                {move || choose(locale.get(), "需要配置", "Requires config")}
                            </span>
                        </Show>
                        <Show when=move || flag_requires_config && !flag_config_ready>
                            <span class="rounded-full bg-rose-100 px-2 py-1 text-rose-800">
                                {move || {
                                    choose(
                                        locale.get(),
                                        "配置缺失，当前不可完整生效",
                                        "Config missing; cannot fully apply",
                                    )
                                }}
                            </span>
                        </Show>
                        <Show when=move || flag_enabled != flag_effective_enabled>
                            <span class="rounded-full bg-amber-100 px-2 py-1 text-amber-800">
                                {move || {
                                    choose(
                                        locale.get(),
                                        "期望状态与当前生效状态不一致",
                                        "Desired state differs from effective state",
                                    )
                                }}
                            </span>
                        </Show>
                        <Show when=move || flag_has_pending_request>
                            <span class="rounded-full bg-amber-100 px-2 py-1 text-amber-800">
                                {move || choose(locale.get(), "有待处理申请", "pending request")}
                            </span>
                        </Show>
                    </div>
                </div>
                <div class="text-right text-xs text-muted-foreground">
                    {flag
                        .updated_at
                        .map(|value| {
                            format!(
                                "{} {}",
                                choose(locale.get(), "更新时间", "updated"),
                                format_unix_timestamp(value)
                            )
                        })
                        .unwrap_or_else(|| choose(locale.get(), "初始值", "seeded").to_string())}
                </div>
            </div>
            <div class="mt-4 flex flex-col gap-2 md:flex-row">
                <input
                    type="text"
                    class="flex-1 rounded border border-border px-3 py-2 text-sm"
                    placeholder={move || {
                        choose(
                            locale.get(),
                            "填写本次变更申请的原因",
                            "Reason for this change request",
                        )
                    }}
                    value=move || reason.get()
                    on:input=move |ev| set_reason.set(event_target_value(&ev))
                />
                <button
                    class="rounded border border-border px-3 py-2 text-sm hover:bg-muted/40 disabled:opacity-50"
                    disabled=move || {
                        reason.get().trim().is_empty()
                            || flag.has_pending_request
                            || busy_action.get() == request_action_key_for_disabled.get_value()
                    }
                    on:click=move |_| {
                        if let Some(token) = auth.token.get() {
                            set_error.set(String::new());
                            set_busy_action.set(request_action_key.get_value());
                            let client = ApiClient::new(api_base_url()).with_auth(token);
                            let flag_key = flag_key.clone();
                            let next_enabled = !flag_enabled;
                            let reason_value = reason.get();
                            let current_locale = locale.get_untracked();
                            spawn(async move {
                                match client
                                    .request_feature_flag_change(
                                        &flag_key,
                                        next_enabled,
                                        &reason_value,
                                    )
                                    .await
                                {
                                    Ok(_) => {
                                        set_reason.set(String::new());
                                        set_reload_nonce.update(|value| *value += 1);
                                    }
                                    Err(error) => set_error.set(format!(
                                        "{}: {}",
                                        choose(
                                            current_locale,
                                            "提交变更申请失败",
                                            "Failed to request feature flag change",
                                        ),
                                        error
                                    )),
                                }
                                set_busy_action.set(String::new());
                            });
                        }
                    }
                >
                    {move || {
                        if flag_enabled {
                            choose(locale.get(), "申请关闭", "Request Disable")
                        } else {
                            choose(locale.get(), "申请开启", "Request Enable")
                        }
                    }}
                </button>
                <button
                    class="rounded border border-danger/30 px-3 py-2 text-sm text-danger hover:bg-danger/10 disabled:opacity-50"
                    disabled=move || {
                        busy_action.get() == override_action_key_for_disabled.get_value()
                    }
                    on:click=move |_| set_show_override_confirm.set(true)
                >
                    {move || choose(locale.get(), "紧急覆盖", "Emergency Override")}
                </button>
            </div>
            <Show when=move || show_override_confirm.get()>
                <div class="mt-3 rounded-lg border border-danger/30 bg-danger/10 px-4 py-4">
                    <div class="text-sm font-medium text-danger">
                        {move || {
                            choose(
                                locale.get(),
                                "直接覆盖会绕过申请/审核流程，仅适合线上应急。",
                                "Direct override bypasses request/review and should be reserved for incidents.",
                            )
                        }}
                    </div>
                    <div class="mt-2 text-xs text-danger">
                        {move || {
                            format!(
                                "{} {}",
                                choose(
                                    locale.get(),
                                    "请输入功能开关键确认：",
                                    "Type the flag key to confirm:",
                                ),
                                flag_key_for_override_prompt.get_value()
                            )
                        }}
                    </div>
                    <input
                        type="text"
                        class="mt-3 w-full rounded border border-danger/30 bg-card px-3 py-2 text-sm"
                        value=move || override_phrase.get()
                        on:input=move |ev| set_override_phrase.set(event_target_value(&ev))
                    />
                    <div class="mt-3 flex flex-wrap gap-2">
                        <button
                            type="button"
                            class="rounded border border-danger/40 bg-danger px-3 py-2 text-sm text-white disabled:opacity-50"
                            disabled=move || {
                                busy_action.get() == override_action_key_for_disabled.get_value()
                                    || override_phrase.get().trim()
                                        != flag_key_for_override_match.get_value()
                            }
                            on:click=move |_| {
                                if let Some(token) = auth.token.get() {
                                    set_error.set(String::new());
                                    set_busy_action.set(override_action_key.get_value());
                                    let client = ApiClient::new(api_base_url()).with_auth(token);
                                    let flag_key = flag_key_for_override_execute.get_value();
                                    let next_enabled = !flag_enabled;
                                    let current_locale = locale.get_untracked();
                                    spawn(async move {
                                        match client.set_feature_flag(&flag_key, next_enabled).await {
                                            Ok(_) => {
                                                set_reload_nonce.update(|value| *value += 1);
                                                set_show_override_confirm.set(false);
                                                set_override_phrase.set(String::new());
                                            }
                                            Err(error) => set_error.set(format!(
                                                "{}: {}",
                                                choose(
                                                    current_locale,
                                                    "紧急覆盖失败",
                                                    "Emergency override failed",
                                                ),
                                                error
                                            )),
                                        }
                                        set_busy_action.set(String::new());
                                    });
                                }
                            }
                        >
                            {move || choose(locale.get(), "确认直接覆盖", "Confirm Override")}
                        </button>
                        <button
                            type="button"
                            class="rounded border border-border bg-card px-3 py-2 text-sm text-foreground"
                            on:click=move |_| {
                                set_show_override_confirm.set(false);
                                set_override_phrase.set(String::new());
                            }
                        >
                            {move || choose(locale.get(), "取消", "Cancel")}
                        </button>
                    </div>
                    <Show when=move || flag_has_pending_request || (flag_requires_config && !flag_config_ready)>
                        <div class="mt-3 text-xs text-danger">
                            {move || {
                                if flag_has_pending_request && flag_requires_config && !flag_config_ready {
                                    choose(
                                        locale.get(),
                                        "当前既有待处理申请，也存在配置缺失；请确认你是在处理线上应急。",
                                        "There is already a pending request and the config is incomplete; confirm this is a real incident.",
                                    )
                                } else if flag_has_pending_request {
                                    choose(
                                        locale.get(),
                                        "当前已有待处理申请，直接覆盖会跳过审核结论。",
                                        "There is already a pending request; a direct override will bypass the review outcome.",
                                    )
                                } else {
                                    choose(
                                        locale.get(),
                                        "当前配置未就绪，直接覆盖后也可能无法完整生效。",
                                        "Configuration is not ready, so the override may still not fully apply.",
                                    )
                                }
                            }}
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}
