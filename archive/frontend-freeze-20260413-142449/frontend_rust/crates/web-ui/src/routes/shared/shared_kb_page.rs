#[component]
pub fn SharedKbPage() -> impl IntoView {
    // Get token from route params
    let params = use_params_map();
    let locale = use_ui_prefs_state().locale;

    if !ui_capabilities().shared_kb {
        return view! {
            <div class="app-page-shell">
                <div class="mx-auto max-w-4xl">
                    <UnavailableFeatureCard
                        title={t(locale.get_untracked(), MessageKey::SharedKbUnavailableTitle).to_string()}
                        description={t(locale.get_untracked(), MessageKey::SharedKbUnavailableDesc).to_string()}
                    />
                </div>
            </div>
        }
        .into_any();
    }

    let token = move || params.get().get("token").unwrap_or_default();

    // State
    let (notebook_name, set_notebook_name) = signal(String::new());
    let (shared_payload, set_shared_payload) = signal(Option::<SharedNotebookPayload>::None);
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(String::new());
    let (query, set_query) = signal(String::new());
    let (answer, set_answer) = signal(String::new());
    let (answering, set_answering) = signal(false);
    let (loaded_shared_key, set_loaded_shared_key) = signal(String::new());
    let (streaming_answer, set_streaming_answer) = signal(String::new());
    let (citations, set_citations) = signal(Vec::<Citation>::new());
    let (chat_sources, set_chat_sources) = signal(Vec::<SourceRef>::new());
    let (degrade_reasons, set_degrade_reasons) = signal(Vec::<String>::new());
    let (result_scroll_top_px, _set_result_scroll_top_px) = signal(0.0);
    let (result_viewport_height_px, _set_result_viewport_height_px) =
        signal(SHARED_VIEWPORT_FALLBACK_PX);
    let result_scroller = NodeRef::<leptos::html::Div>::new();
    let prompt_suggestions = move || {
        vec![
            t(locale.get(), MessageKey::SharedKbPromptTopic).to_string(),
            t(locale.get(), MessageKey::SharedKbPromptTakeaways).to_string(),
            t(locale.get(), MessageKey::SharedKbPromptFacts).to_string(),
        ]
    };
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(container) = result_scroller.get() {
                _set_result_viewport_height_px.set(container.client_height() as f64);
            }
        }
    });
    let shared_virtual_items = Signal::derive(move || {
        let mut items = Vec::new();
        let answer_text = shared_answer_item_text(&streaming_answer.get(), &answer.get());
        if !answer_text.is_empty() {
            items.push(HeightState::predicted("shared-answer", 240.0));
        }
        if !citations.get().is_empty() {
            items.push(HeightState::predicted("shared-citations", 180.0));
        }
        if !chat_sources.get().is_empty() {
            items.push(HeightState::predicted("shared-sources", 180.0));
        }
        items
    });
    let visible_shared_ids = Signal::derive(move || {
        compute_window(
            &shared_virtual_items.get(),
            result_scroll_top_px.get(),
            result_viewport_height_px.get(),
            SHARED_LIST_OVERSCAN,
        )
        .visible_ids
        .into_iter()
        .collect::<HashSet<_>>()
    });
    run_once_after_hydration(
        move || token(),
        loaded_shared_key,
        set_loaded_shared_key,
        move || {
            spawn(async move {
                let tok = token();
                if tok.is_empty() {
                    set_error
                        .set(t(locale.get_untracked(), MessageKey::SharedKbInvalidLink).to_string());
                    set_loading.set(false);
                    return;
                }

                let client = ApiClient::new(api_base_url());

                match client.get_shared_kb(&tok).await {
                    Ok(payload) => {
                        set_notebook_name.set(payload.knowledge_base.title.clone());
                        set_shared_payload.set(Some(payload));
                    }
                    Err(e) => {
                        set_error.set(format!(
                            "{}: {}",
                            t(locale.get_untracked(), MessageKey::SharedKbInvalidOrExpired),
                            e
                        ));
                    }
                }
                set_loading.set(false);
            });
        },
    );

    let handle_query = move |ev: SubmitEvent| {
        ev.prevent_default();
        let query_val = query.get();
        if query_val.trim().is_empty() {
            return;
        }

        let tok = token();
        let request_id = next_request_id();
        set_answering.set(true);
        set_answer.set(String::new());
        set_streaming_answer.set(String::new());
        set_citations.set(Vec::new());
        set_chat_sources.set(Vec::new());
        set_degrade_reasons.set(Vec::new());

        let client = ChatSseClient::new(api_base_url());

        spawn(async move {
            let shared_notebook_id = shared_payload
                .get_untracked()
                .map(|payload| payload.knowledge_base.id)
                .unwrap_or_default();
            match client
                .stream_chat_with_request(web_sdk::dtos::ChatRequest {
                    query: query_val.clone(),
                    notebook_id: (!shared_notebook_id.is_empty())
                        .then_some(shared_notebook_id.clone()),
                    session_id: None,
                    agent_type: "rag".to_string(),
                    source_type: Some("share".to_string()),
                    source_token: Some(tok.clone()),
                    doc_scope: vec![],
                    messages: vec![],
                    stream: true,
                }, Some(request_id.as_str()))
                .await
            {
                Ok(mut stream) => {
                    let mut current_answer = String::new();
                    let mut current_citations = Vec::<Citation>::new();
                    while let Some(event) = stream.next().await {
                        match event {
                            SseEvent::Token { content, .. } => {
                                current_answer.push_str(&content);
                                set_streaming_answer.set(current_answer.clone());
                            }
                            SseEvent::Citations { citations: next, .. } => {
                                current_citations = typed_citations_from_values(next);
                                set_chat_sources
                                    .set(shared_chat_sources_from_citations(&current_citations));
                                set_citations.set(current_citations.clone());
                            }
                            SseEvent::Done { payload, .. } => {
                                if let Some(done_citations) = payload
                                    .get("citations")
                                    .and_then(|value| value.as_array())
                                    .map(|items| typed_citations_from_values(items.clone()))
                                {
                                    current_citations = done_citations;
                                    set_citations.set(current_citations.clone());
                                }
                                set_chat_sources
                                    .set(shared_chat_sources_from_citations(&current_citations));
                                set_answer
                                    .set(payload_answer(&payload).unwrap_or_else(|| current_answer.clone()));
                                set_streaming_answer.set(String::new());
                                set_degrade_reasons.set(payload_degrade_reasons(&payload));
                                set_answering.set(false);
                                break;
                            }
                            SseEvent::Error { message, .. } => {
                                set_streaming_answer.set(String::new());
                                set_error.set(format!(
                                    "{}: {}",
                                    t(locale.get_untracked(), MessageKey::SharedKbAnswerFailed),
                                    message
                                ));
                                set_answering.set(false);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    set_error.set(format!(
                        "{}: {}",
                        t(locale.get_untracked(), MessageKey::SharedKbAnswerFailed),
                        e
                    ));
                    set_answering.set(false);
                }
            }
        });
    };

    view! {
        <div class="app-page-shell">
            <div class="mx-auto max-w-4xl space-y-6">
                <div class="app-page-heading mb-0">
                    <A href="/" attr:class="app-link inline-flex items-center gap-1">
                        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"/>
                        </svg>
                        {move || t(locale.get(), MessageKey::SharedKbHome)}
                    </A>
                    <h1 class="app-page-title">
                        {notebook_name.get()}
                    </h1>
                    <p class="app-page-subtitle">
                        {move || t(locale.get(), MessageKey::SharedKbSubtitle)}
                    </p>
                </div>

                <Show when=move || !error.get().is_empty()>
                    <NoticeBanner message=error.get() tone=NoticeTone::Danger />
                </Show>

                <Show when=move || loading.get()>
                    <div class="app-empty-state">
                        {move || t(locale.get(), MessageKey::SharedKbLoading)}
                    </div>
                </Show>

                <Show when=move || !loading.get() && error.get().is_empty()>
                    <div class="space-y-4">
                        <SharedKbOverview locale=locale shared_payload=shared_payload />

                        <div class="app-surface-card">
                            <div class="mb-6">
                                <h2 class="mb-2 text-lg font-medium text-card-foreground">
                                    {move || t(locale.get(), MessageKey::SharedKbChatTitle)}
                                </h2>
                                <p class="text-sm text-muted-foreground">
                                    {move || t(locale.get(), MessageKey::SharedKbChatDesc)}
                                </p>
                            </div>

                            <div class="mb-6 rounded-xl border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-900">
                                {move || t(locale.get(), MessageKey::SharedKbSuggestionHint)}
                            </div>

                            <div class="mb-6 flex flex-wrap gap-2">
                                {prompt_suggestions().into_iter().map(|suggestion| {
                                    let suggestion_value = suggestion.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="rounded-full border border-border bg-card px-3 py-1.5 text-sm text-foreground hover:border-primary/40 hover:text-primary"
                                            on:click=move |_| set_query.set(suggestion_value.clone())
                                        >
                                            {suggestion}
                                        </button>
                                    }
                                }).collect_view()}
                            </div>

                        {/* Query form */}
                        <form on:submit=handle_query class="mb-6">
                            <div class="flex gap-2">
                                <input
                                    type="text"
                                    class="app-input flex-1"
                                    placeholder={move || t(locale.get(), MessageKey::SharedKbInputPlaceholder)}
                                    value=query.get()
                                    on:input=move |ev| set_query.set(event_target_value(&ev))
                                    disabled=answering.get()
                                />
                                <button
                                    type="submit"
                                    class="app-button-primary"
                                    disabled=answering.get()
                                >
                                    {move || if answering.get() {
                                        t(locale.get(), MessageKey::SharedKbThinking)
                                    } else {
                                        t(locale.get(), MessageKey::SharedKbAsk)
                                    }}
                                </button>
                            </div>
                        </form>

                        {/* Answer */}
                        <Show when=move || !degrade_reasons.get().is_empty()>
                            <div class="mb-4 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
                                <div class="font-medium">{move || t(locale.get(), MessageKey::SharedKbDegraded)}</div>
                                <div class="mt-1">{degrade_reasons.get().join(" | ")}</div>
                            </div>
                        </Show>

                        <Show when=move || !shared_virtual_items.get().is_empty()>
                            <div class="border-t border-border pt-4">
                                <div class="mb-2 flex items-center justify-between">
                                    <h3 class="text-sm font-medium text-foreground">
                                        {move || t(locale.get(), MessageKey::SharedKbAnswerBlockTitle)}
                                    </h3>
                                    <span class="text-xs text-muted-foreground">{shared_virtual_items.get().len()}</span>
                                </div>
                                <div
                                    class="max-h-[70vh] overflow-y-auto pr-1"
                                    node_ref=result_scroller
                                    data-test-shared-scroll
                                    on:scroll=move |_ev| {
                                        #[cfg(target_arch = "wasm32")]
                                        {
                                            let container: web_sys::HtmlElement = event_target(&_ev);
                                            _set_result_scroll_top_px.set(container.scroll_top() as f64);
                                            _set_result_viewport_height_px.set(container.client_height() as f64);
                                        }
                                    }
                                >
                                    <VirtualTextList
                                        row_heights=Signal::derive(move || shared_virtual_items.get())
                                        viewport_height_px=Signal::derive(move || result_viewport_height_px.get())
                                        scroll_top_px=Signal::derive(move || result_scroll_top_px.get())
                                        overscan=SHARED_LIST_OVERSCAN
                                    >
                                        <div class="space-y-4">
                                            {move || {
                                                let visible_ids = visible_shared_ids.get();
                                                let mut items = Vec::new();

                                                if visible_ids.contains("shared-answer") {
                                                    let answer_text =
                                                        shared_answer_item_text(&streaming_answer.get(), &answer.get());
                                                    items.push(
                                                        view! {
                                                            <div class="rounded-xl border border-border bg-card px-3 py-3">
                                                                <div class="mb-2 flex items-center justify-between">
                                                                    <h4 class="text-sm font-medium text-foreground">
                                                                        {move || t(locale.get(), MessageKey::SharedKbAnswerLabel)}
                                                                    </h4>
                                                                    <span class="text-xs text-muted-foreground">
                                                                        {move || t(locale.get(), MessageKey::SharedKbLive)}
                                                                    </span>
                                                                </div>
                                                                <div class="prose prose-sm max-w-none whitespace-pre-wrap text-foreground">
                                                                    {answer_text}
                                                                </div>
                                                            </div>
                                                        }
                                                        .into_any(),
                                                    );
                                                }

                                                if visible_ids.contains("shared-citations") {
                                                    items.push(
                                                        view! {
                                                            <div class="rounded-xl border border-border bg-card px-3 py-3">
                                                                <div class="mb-2 flex items-center justify-between">
                                                                    <h4 class="text-sm font-medium text-foreground">
                                                                        {move || t(locale.get(), MessageKey::SharedKbCitations)}
                                                                    </h4>
                                                                    <span class="text-xs text-muted-foreground">{citations.get().len()}</span>
                                                                </div>
                                                                <div class="space-y-2">
                                                                    {citations.get().into_iter().map(|citation| {
                                                                        let preview_text = shared_source_preview_text(
                                                                            citation.preview.as_deref(),
                                                                            citation.content.as_deref(),
                                                                        );
                                                                        let preview_visible = !preview_text.is_empty();
                                                                        let layer = citation.layer.clone();
                                                                        view! {
                                                                            <div class="rounded-xl border border-border bg-card px-3 py-3">
                                                                                <div class="text-sm font-medium text-card-foreground">{citation.doc_name}</div>
                                                                                <div class="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                                                                                    {if let Some(page) = citation.page {
                                                                                        view! { <span class="rounded-full bg-muted px-2 py-1">{format!("{} {}", t(locale.get(), MessageKey::SearchPage), page)}</span> }.into_any()
                                                                                    } else {
                                                                                        view! { <></> }.into_any()
                                                                                    }}
                                                                                    {if let Some(value) = layer.as_ref() {
                                                                                        view! { <span class="rounded-full bg-muted px-2 py-1">{value.clone()}</span> }.into_any()
                                                                                    } else {
                                                                                        view! { <></> }.into_any()
                                                                                    }}
                                                                                    <span class="rounded-full bg-muted px-2 py-1">{format!("{} {:.2}", t(locale.get(), MessageKey::SharedKbScore), citation.score)}</span>
                                                                                </div>
                                                                                <Show when=move || preview_visible>
                                                                                    <div class="mt-1 text-xs text-muted-foreground">{preview_text.clone()}</div>
                                                                                </Show>
                                                                            </div>
                                                                        }
                                                                    }).collect_view()}
                                                                </div>
                                                            </div>
                                                        }
                                                        .into_any(),
                                                    );
                                                }

                                                if visible_ids.contains("shared-sources") {
                                                    items.push(
                                                        view! {
                                                            <div class="rounded-xl border border-border bg-card px-3 py-3">
                                                                <div class="mb-2 flex items-center justify-between">
                                                                    <h4 class="text-sm font-medium text-foreground">
                                                                        {move || t(locale.get(), MessageKey::SharedKbRetrievedSources)}
                                                                    </h4>
                                                                    <span class="text-xs text-muted-foreground">{chat_sources.get().len()}</span>
                                                                </div>
                                                                <div class="space-y-2">
                                                                    {chat_sources.get().into_iter().map(|source| {
                                                                        let preview_text =
                                                                            shared_source_preview_text(source.snippet.as_deref(), None);
                                                                        let preview_visible = !preview_text.is_empty();
                                                                        view! {
                                                                            <div class="rounded-xl border border-border bg-card px-3 py-3">
                                                                                <div class="text-sm font-medium text-card-foreground">{source.title}</div>
                                                                                <div class="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                                                                                    {if let Some(page) = source.page {
                                                                                        view! { <span class="rounded-full bg-muted px-2 py-1">{format!("{} {}", t(locale.get(), MessageKey::SearchPage), page)}</span> }.into_any()
                                                                                    } else {
                                                                                        view! { <></> }.into_any()
                                                                                    }}
                                                                                </div>
                                                                                <Show when=move || preview_visible>
                                                                                    <div class="mt-1 text-xs text-muted-foreground">{preview_text.clone()}</div>
                                                                                </Show>
                                                                            </div>
                                                                        }
                                                                    }).collect_view()}
                                                                </div>
                                                            </div>
                                                        }
                                                        .into_any(),
                                                    );
                                                }

                                                items.into_iter().collect_view()
                                            }}
                                        </div>
                                    </VirtualTextList>
                                </div>
                            </div>
                        </Show>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
    .into_any()
}
