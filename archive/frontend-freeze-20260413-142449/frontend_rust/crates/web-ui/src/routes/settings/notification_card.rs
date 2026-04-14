#[component]
fn NotificationCard(
    item: NotificationRow,
    set_pending_mark_read: WriteSignal<Option<String>>,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let unread = item.read_at.is_none();
    let notification_id = StoredValue::new(item.id.clone());

    view! {
        <div class="rounded-xl border border-border bg-card p-4">
            <div class="flex items-start justify-between gap-4">
                <div class="min-w-0">
                    <div class="flex items-center gap-2">
                        <h4 class="text-sm font-semibold text-card-foreground">{item.title.clone()}</h4>
                        <Show when=move || unread>
                            <span class="app-status-badge bg-primary/10 text-primary">
                                {move || choose(locale.get(), "未读", "Unread")}
                            </span>
                        </Show>
                    </div>
                    <p class="mt-1 text-sm text-muted-foreground">{item.body.clone()}</p>
                    <p class="mt-2 text-xs text-muted-foreground">{item.event_type.clone()}</p>
                </div>
                <Show when=move || unread>
                    <button
                        class="app-button-secondary shrink-0 px-3 py-1.5 text-xs"
                        on:click=move |_| set_pending_mark_read.set(Some(notification_id.get_value()))
                    >
                        {move || choose(locale.get(), "标记已读", "Mark Read")}
                    </button>
                </Show>
            </div>
        </div>
    }
}
