#[component]
fn FeatureFlagRequestCard(
    request: FeatureFlagChangeRequest,
    locale: ReadSignal<Locale>,
    auth: crate::state::auth::AuthState,
    busy_action: ReadSignal<String>,
    set_busy_action: WriteSignal<String>,
    set_error: WriteSignal<String>,
    set_reload_nonce: WriteSignal<u64>,
) -> impl IntoView {
    let request_id = StoredValue::new(request.id.clone());
    let (review_note, set_review_note) = signal(String::new());
    let request_id_for_approve = request_id;
    let request_id_for_reject = request_id;
    let request_review_note = request.review_note.clone();
    let request_review_note_for_show = request.review_note.clone();
    let request_reviewed_by = request.reviewed_by.clone();
    let request_status = request.status.clone();
    let request_status_for_class = request.status.clone();
    let request_status_for_pending = request.status.clone();
    let request_reviewed_by_for_show = request.reviewed_by.clone();
    let request_created_at = request.created_at;
    let request_reviewed_at = request.reviewed_at;
    let request_executed_at = request.executed_at;
    let current_enabled = request.current_enabled;
    let requested_enabled = request.requested_enabled;

    view! {
        <div class="rounded-lg border border-border px-4 py-4">
            <div class="flex items-start justify-between gap-4">
                <div>
                    <div class="flex flex-wrap items-center gap-2">
                        <div class="font-medium text-foreground">{request.flag_key.clone()}</div>
                        <span class=move || format!(
                            "inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium {}",
                            feature_flag_status_classes(&request_status_for_class)
                        )>
                            {feature_flag_status_label(locale.get(), &request_status)}
                        </span>
                    </div>
                    <div class="mt-3 flex flex-wrap gap-2 text-xs">
                        <span class="rounded-full bg-slate-100 px-2 py-1 text-slate-700">
                            {move || choose(locale.get(), "当前：", "Current: ")}
                            {feature_flag_toggle_label(locale.get(), current_enabled)}
                        </span>
                        <span class="rounded-full bg-sky-100 px-2 py-1 text-sky-800">
                            {move || choose(locale.get(), "申请变更为：", "Requested: ")}
                            {feature_flag_toggle_label(locale.get(), requested_enabled)}
                        </span>
                    </div>
                    <div class="mt-3 rounded-lg bg-slate-50 px-3 py-3 text-sm text-slate-700">
                        {request.reason.clone()}
                    </div>
                    <div class="mt-3 flex flex-wrap gap-x-4 gap-y-2 text-xs text-muted-foreground">
                        <span>
                            {move || choose(locale.get(), "申请人：", "Requested by: ")}
                            {request.requested_by.clone()}
                        </span>
                        <span>
                            {move || choose(locale.get(), "申请时间：", "Created: ")}
                            {format_unix_timestamp(request_created_at)}
                        </span>
                        <Show when=move || request_reviewed_by.is_some()>
                            <span>
                                {move || choose(locale.get(), "审核人：", "Reviewed by: ")}
                                {request_reviewed_by_for_show.clone().unwrap_or_default()}
                            </span>
                        </Show>
                        <Show when=move || request_reviewed_at.is_some()>
                            <span>
                                {move || choose(locale.get(), "审核时间：", "Reviewed at: ")}
                                {request_reviewed_at.map(format_unix_timestamp).unwrap_or_default()}
                            </span>
                        </Show>
                        <Show when=move || request_executed_at.is_some()>
                            <span>
                                {move || choose(locale.get(), "执行时间：", "Executed at: ")}
                                {request_executed_at.map(format_unix_timestamp).unwrap_or_default()}
                            </span>
                        </Show>
                    </div>
                    <Show when=move || request_review_note.is_some()>
                        <div class="mt-2 rounded-lg border border-slate-200 bg-card px-3 py-2 text-xs text-slate-600">
                            {move || choose(locale.get(), "审核备注：", "Review note: ")}
                            {request_review_note_for_show.clone().unwrap_or_default()}
                        </div>
                    </Show>
                </div>
                <div class="text-xs text-muted-foreground">{format!("#{}", request.id)}</div>
            </div>
            <Show when=move || request_status_for_pending == "pending">
                <div class="mt-4 flex flex-col gap-2 md:flex-row">
                    <input
                        type="text"
                        class="flex-1 rounded border border-border px-3 py-2 text-sm"
                        placeholder={move || choose(locale.get(), "可选：填写审核备注", "Optional review note")}
                        value=move || review_note.get()
                        on:input=move |ev| set_review_note.set(event_target_value(&ev))
                    />
                    <button
                        class="rounded border border-green-200 px-3 py-2 text-sm text-success hover:bg-success/10 disabled:opacity-50"
                        disabled=move || {
                            busy_action.get() == format!("approve:{}", request_id.get_value())
                        }
                        on:click=move |_| {
                            if let Some(token) = auth.token.get() {
                                set_error.set(String::new());
                                let action_key =
                                    format!("approve:{}", request_id_for_approve.get_value());
                                set_busy_action.set(action_key.clone());
                                let client = ApiClient::new(api_base_url()).with_auth(token);
                                let request_id = request_id_for_approve.get_value();
                                let note = review_note.get();
                                let current_locale = locale.get_untracked();
                                spawn(async move {
                                    match client
                                        .review_feature_flag_change(
                                            &request_id,
                                            true,
                                            (!note.trim().is_empty()).then_some(note.as_str()),
                                        )
                                        .await
                                    {
                                        Ok(_) => set_reload_nonce.update(|value| *value += 1),
                                        Err(error) => set_error.set(format!(
                                            "{}: {}",
                                            choose(
                                                current_locale,
                                                "批准变更申请失败",
                                                "Failed to approve feature flag request",
                                            ),
                                            error
                                        )),
                                    }
                                    set_busy_action.set(String::new());
                                });
                            }
                        }
                    >
                        {move || choose(locale.get(), "批准并执行", "Approve & Execute")}
                    </button>
                    <button
                        class="rounded border border-danger/30 px-3 py-2 text-sm text-danger hover:bg-danger/10 disabled:opacity-50"
                        disabled=move || {
                            busy_action.get() == format!("reject:{}", request_id.get_value())
                        }
                        on:click=move |_| {
                            if let Some(token) = auth.token.get() {
                                set_error.set(String::new());
                                let action_key =
                                    format!("reject:{}", request_id_for_reject.get_value());
                                set_busy_action.set(action_key.clone());
                                let client = ApiClient::new(api_base_url()).with_auth(token);
                                let request_id = request_id_for_reject.get_value();
                                let note = review_note.get();
                                let current_locale = locale.get_untracked();
                                spawn(async move {
                                    match client
                                        .review_feature_flag_change(
                                            &request_id,
                                            false,
                                            (!note.trim().is_empty()).then_some(note.as_str()),
                                        )
                                        .await
                                    {
                                        Ok(_) => set_reload_nonce.update(|value| *value += 1),
                                        Err(error) => set_error.set(format!(
                                            "{}: {}",
                                            choose(
                                                current_locale,
                                                "拒绝变更申请失败",
                                                "Failed to reject feature flag request",
                                            ),
                                            error
                                        )),
                                    }
                                    set_busy_action.set(String::new());
                                });
                            }
                        }
                    >
                        {move || choose(locale.get(), "拒绝", "Reject")}
                    </button>
                </div>
            </Show>
        </div>
    }
}
