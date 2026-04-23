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
            <div class={format!("{} {}", shared_page_style::card, shared_page_style::card_pad)}>
                <div class=shared_page_style::section_intro>
                    <h2 class=shared_page_style::section_title>
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
                        <p class=shared_page_style::section_desc>
                            {shared_payload
                                .get()
                                .as_ref()
                                .and_then(|payload| payload.knowledge_base.description.clone())
                            .unwrap_or_default()}
                        </p>
                    </Show>
                </div>
                <div class=shared_page_style::overview_meta>
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
                <div class=shared_page_style::metric_grid>
                    <div class=shared_page_style::metric_card>
                        <div class=shared_page_style::metric_label>
                            {move || choose(locale.get(), "可用资料", "Ready sources")}
                        </div>
                        <div class=shared_page_style::metric_value>
                            {move || ready_source_count().to_string()}
                        </div>
                    </div>
                    <div class=shared_page_style::metric_card>
                        <div class=shared_page_style::metric_label>
                            {move || choose(locale.get(), "待处理资料", "Pending sources")}
                        </div>
                        <div class=shared_page_style::metric_value>
                            {move || pending_source_count().to_string()}
                        </div>
                    </div>
                    <div class=shared_page_style::metric_card>
                        <div class=shared_page_style::metric_label>
                            {move || choose(locale.get(), "下载策略", "Download policy")}
                        </div>
                        <div class=shared_page_style::metric_value_compact>
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
            <div class={format!("{} {}", shared_page_style::card, shared_page_style::card_pad)}>
                <h2 class=shared_page_style::section_title>
                    {move || choose(locale.get(), "共享资料", "Shared Sources")}
                </h2>
                <div class=shared_page_style::source_list>
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
                                        <div class=shared_page_style::source_row>
                                            <div class=shared_page_style::item_title>
                                                {source.file_name}
                                            </div>
                                            <div class=shared_page_style::source_status>
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
