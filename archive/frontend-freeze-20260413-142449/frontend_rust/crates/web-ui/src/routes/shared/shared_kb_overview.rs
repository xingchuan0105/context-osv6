#[component]
fn SharedKbOverview(
    locale: ReadSignal<crate::i18n::Locale>,
    shared_payload: ReadSignal<Option<SharedNotebookPayload>>,
) -> impl IntoView {
    let ready_source_count = move || {
        shared_payload
            .get()
            .map(|payload| {
                payload
                    .sources
                    .iter()
                    .filter(|source| matches!(source.status.as_str(), "ready" | "completed"))
                    .count()
            })
            .unwrap_or_default()
    };
    let pending_source_count = move || {
        shared_payload
            .get()
            .map(|payload| {
                payload
                    .sources
                    .iter()
                    .filter(|source| !matches!(source.status.as_str(), "ready" | "completed"))
                    .count()
            })
            .unwrap_or_default()
    };

    view! {
        <Show when=move || shared_payload.get().is_some()>
            <div class="app-surface-card">
                <div class="mb-3">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {shared_payload
                            .get()
                            .as_ref()
                            .map(|payload| payload.knowledge_base.title.clone())
                            .unwrap_or_default()}
                    </h2>
                    <Show when=move || {
                        shared_payload
                            .get()
                            .as_ref()
                            .and_then(|payload| payload.knowledge_base.description.clone())
                            .is_some()
                    }>
                        <p class="mt-1 text-sm text-muted-foreground">
                            {shared_payload
                                .get()
                                .as_ref()
                                .and_then(|payload| payload.knowledge_base.description.clone())
                                .unwrap_or_default()}
                        </p>
                    </Show>
                </div>
                <div class="flex flex-wrap gap-3 text-xs text-muted-foreground">
                    <span>
                        {move || choose(locale.get(), "权限：", "Permission: ")}
                        {shared_payload
                            .get()
                            .as_ref()
                            .map(|payload| permission_label(locale.get(), &payload.share.permission))
                            .unwrap_or_default()}
                    </span>
                    <span>
                        {move || choose(locale.get(), "过期时间：", "Expires: ")}
                        {shared_payload
                            .get()
                            .as_ref()
                            .and_then(|payload| payload.share.expires_at.clone())
                            .unwrap_or_else(|| choose(locale.get(), "永不过期", "never").to_string())}
                    </span>
                    <span>
                        {move || choose(locale.get(), "资料数：", "Sources: ")}
                        {shared_payload
                            .get()
                            .as_ref()
                            .map(|payload| payload.sources.len())
                            .unwrap_or(0)}
                    </span>
                    <span>
                        {move || choose(locale.get(), "下载：", "Downloads: ")}
                        {shared_payload
                            .get()
                            .as_ref()
                            .map(|payload| {
                                if payload.share.allow_download {
                                    choose(locale.get(), "允许", "Allowed")
                                } else {
                                    choose(locale.get(), "关闭", "Disabled")
                                }
                            })
                            .unwrap_or_else(|| choose(locale.get(), "关闭", "Disabled"))}
                    </span>
                    <span>
                        {move || choose(locale.get(), "权限范围：", "Scope: ")}
                        {shared_payload
                            .get()
                            .as_ref()
                            .map(|payload| {
                                if payload.share.scope == "partial" {
                                    choose(locale.get(), "仅预览", "Preview only")
                                } else {
                                    choose(locale.get(), "完整访问", "Full access")
                                }
                            })
                            .unwrap_or_else(|| choose(locale.get(), "完整访问", "Full access"))}
                    </span>
                </div>
                <div class="mt-4 grid gap-3 md:grid-cols-3">
                    <div class="app-metric-card">
                        <div class="text-xs text-muted-foreground">
                            {move || choose(locale.get(), "可用资料", "Ready sources")}
                        </div>
                        <div class="mt-2 text-xl font-semibold text-card-foreground">
                            {move || ready_source_count().to_string()}
                        </div>
                    </div>
                    <div class="app-metric-card">
                        <div class="text-xs text-muted-foreground">
                            {move || choose(locale.get(), "待处理资料", "Pending sources")}
                        </div>
                        <div class="mt-2 text-xl font-semibold text-card-foreground">
                            {move || pending_source_count().to_string()}
                        </div>
                    </div>
                    <div class="app-metric-card">
                        <div class="text-xs text-muted-foreground">
                            {move || choose(locale.get(), "下载策略", "Download policy")}
                        </div>
                        <div class="mt-2 text-sm font-semibold text-card-foreground">
                            {shared_payload
                                .get()
                                .as_ref()
                                .map(|payload| {
                                    if payload.share.allow_download {
                                        choose(locale.get(), "允许下载原始资料", "Source downloads enabled")
                                    } else {
                                        choose(locale.get(), "仅在线查看", "View only")
                                    }
                                })
                                .unwrap_or_else(|| choose(locale.get(), "仅在线查看", "View only"))}
                        </div>
                    </div>
                </div>
            </div>
        </Show>

        <Show when=move || {
            shared_payload
                .get()
                .as_ref()
                .map(|payload| !payload.sources.is_empty())
                .unwrap_or(false)
        }>
            <div class="app-surface-card">
                <h2 class="mb-3 text-lg font-semibold text-card-foreground">
                    {move || choose(locale.get(), "共享资料", "Shared Sources")}
                </h2>
                <div class="space-y-2">
                    {shared_payload
                        .get()
                        .as_ref()
                        .map(|payload| {
                            payload
                                .sources
                                .iter()
                                .cloned()
                                .map(|source| {
                                    view! {
                                        <div class="rounded-xl border border-border bg-card px-3 py-2">
                                            <div class="text-sm font-medium text-card-foreground">
                                                {source.file_name}
                                            </div>
                                            <div class="mt-1 text-xs text-muted-foreground">
                                                {source_status_label(locale.get(), &source.status)}
                                            </div>
                                        </div>
                                    }
                                })
                                .collect_view()
                        })
                        .unwrap_or_default()}
                </div>
            </div>
        </Show>
    }
}
