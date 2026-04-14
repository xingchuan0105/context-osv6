//! Document components - Upload, Detail, and ListItem for document management

use leptos::html::Input;
use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos::task::spawn_local;
use reqwest::Client as HttpClient;
use web_sdk::ApiClient;
use web_sdk::dtos::{CreateDocumentRequest, ParsedPreviewItem, SourceRow};

use crate::api::api_base_url;
use crate::components::{NoticeBanner, NoticeTone, UnavailableFeatureCard};
use crate::i18n::{Locale, choose};
use crate::load::run_once_after_hydration;
use crate::platform::ui_capabilities;
use crate::state::auth::use_auth_state;
use crate::state::chat::use_chat_state;
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::workspace::{CitationFocus, use_workspace_state};

/// Returns the CSS color class for a document status
fn status_color(status: &str) -> &'static str {
    match status {
        "completed" | "ready" => "bg-green-100 text-green-800",
        "pending" | "enqueueing" | "queued" => "bg-slate-100 text-slate-800",
        "processing" => "bg-yellow-100 text-yellow-800",
        "failed" | "error" => "bg-red-100 text-red-800",
        _ => "bg-gray-100 text-gray-800",
    }
}

fn status_label(locale: Locale, status: &str) -> String {
    match status {
        "completed" | "ready" => choose(locale, "可用", "Ready").to_string(),
        "pending" | "enqueueing" | "queued" => choose(locale, "排队中", "Queued").to_string(),
        "processing" => choose(locale, "处理中", "Processing").to_string(),
        "failed" | "error" => choose(locale, "失败", "Failed").to_string(),
        _ => status.to_string(),
    }
}

#[cfg(target_arch = "wasm32")]
async fn browser_file_to_bytes(file: web_sys::File) -> Result<Vec<u8>, String> {
    use js_sys::Uint8Array;
    use wasm_bindgen_futures::JsFuture;

    let promise = file.array_buffer();
    let value = JsFuture::from(promise)
        .await
        .map_err(|error| format!("failed to read file: {:?}", error))?;
    let bytes = Uint8Array::new(&value);
    let mut output = vec![0; bytes.length() as usize];
    bytes.copy_to(&mut output);
    Ok(output)
}

#[cfg(not(target_arch = "wasm32"))]
async fn browser_file_to_bytes(_file: web_sys::File) -> Result<Vec<u8>, String> {
    Err("file upload is only supported in the browser runtime".to_string())
}

fn preview_item_dom_id(source_id: &str, item: &ParsedPreviewItem) -> String {
    format!("parsed-preview-{}-{}-{}", source_id, item.page, item.cursor)
}

fn citation_focus_matches_item(focus: &CitationFocus, item: &ParsedPreviewItem) -> bool {
    if let Some(page) = focus.page
        && item.page != page
    {
        return false;
    }

    let preview_match = focus
        .preview
        .as_ref()
        .map(|preview| item.text.contains(preview) || preview.contains(&item.text))
        .unwrap_or(false);
    let content_match = focus
        .content
        .as_ref()
        .map(|content| item.text.contains(content) || content.contains(&item.text))
        .unwrap_or(false);

    preview_match || content_match || focus.page.is_some_and(|page| page == item.page)
}

#[cfg(target_arch = "wasm32")]
fn scroll_preview_item_into_view(element_id: &str) {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(element) = document.get_element_by_id(element_id) else {
        return;
    };
    element.scroll_into_view();
    if let Ok(html) = element.dyn_into::<web_sys::HtmlElement>() {
        let _ = html.focus();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn scroll_preview_item_into_view(_element_id: &str) {}

fn append_preview_page(
    auth_token: ReadSignal<Option<String>>,
    locale: ReadSignal<Locale>,
    document_id: String,
    content: ReadSignal<String>,
    set_content: WriteSignal<String>,
    loading_more_preview: ReadSignal<bool>,
    set_loading_more_preview: WriteSignal<bool>,
    next_preview_cursor: ReadSignal<usize>,
    set_next_preview_cursor: WriteSignal<usize>,
    preview_has_more: ReadSignal<bool>,
    set_preview_has_more: WriteSignal<bool>,
    set_parsed_preview: WriteSignal<Vec<ParsedPreviewItem>>,
    set_content_error: WriteSignal<String>,
) {
    if loading_more_preview.get_untracked() || !preview_has_more.get_untracked() {
        return;
    }
    let Some(token) = auth_token.get() else {
        return;
    };
    let locale_now = locale.get_untracked();
    let cursor = next_preview_cursor.get_untracked();
    set_loading_more_preview.set(true);
    let client = ApiClient::new(api_base_url()).with_auth(token);

    spawn(async move {
        match client.get_parsed_preview(&document_id, cursor, 120).await {
            Ok(resp) => {
                if let Some(summary) = resp.summary
                    && content.get_untracked().is_empty()
                    && !summary.is_empty()
                {
                    set_content.set(summary);
                }
                set_parsed_preview.update(|items| items.extend(resp.items));
                set_preview_has_more.set(resp.has_more);
                set_next_preview_cursor.set(resp.next_cursor);
            }
            Err(e) => {
                set_preview_has_more.set(false);
                set_content_error.set(format!(
                    "{}: {}",
                    choose(
                        locale_now,
                        "加载更多预览失败",
                        "Failed to load more preview"
                    ),
                    e
                ));
            }
        }
        set_loading_more_preview.set(false);
    });
}

/// DocumentUpload component - handles file selection
#[component]
pub fn DocumentUpload(
    notebook_id: String,
    on_upload_success: impl Fn(String) + 'static,
    on_cancel_request: impl Fn() + 'static,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;

    if !ui_capabilities().document_upload {
        return view! {
            <UnavailableFeatureCard
                title={choose(locale.get_untracked(), "上传文档", "Upload Document").to_string()}
                description={choose(
                    locale.get_untracked(),
                    "文档上传在当前版本中暂不可用。",
                    "Document upload is unavailable in this build."
                ).to_string()}
            />
        }
        .into_any();
    }

    let on_upload_success = std::rc::Rc::new(on_upload_success);
    let auth = use_auth_state();
    let input_ref = NodeRef::<Input>::new();
    let (selected_filename, set_selected_filename) = signal(String::new());
    let (selected_file, set_selected_file) = signal(Option::<web_sys::File>::None);
    let (uploading, set_uploading) = signal(false);
    let (error, set_error) = signal(String::new());

    let handle_change = move |_| {
        let Some(input) = input_ref.get() else {
            return;
        };
        let Some(file_list) = input.files() else {
            return;
        };
        let Some(file) = file_list.get(0) else {
            set_selected_file.set(None);
            set_selected_filename.set(String::new());
            return;
        };
        set_selected_filename.set(file.name());
        set_selected_file.set(Some(file));
        set_error.set(String::new());
    };

    let handle_upload = move |_| {
        let token = match auth.token.get() {
            Some(token) => token,
            None => {
                set_error.set(
                    choose(locale.get_untracked(), "尚未登录", "Not authenticated").to_string(),
                );
                return;
            }
        };

        let Some(file) = selected_file.get() else {
            set_error.set(
                choose(
                    locale.get_untracked(),
                    "请先选择文件",
                    "Please select a file",
                )
                .to_string(),
            );
            return;
        };

        set_uploading.set(true);
        set_error.set(String::new());

        let notebook_id = notebook_id.clone();
        let on_upload_success = on_upload_success.clone();

        spawn_local(async move {
            let client = ApiClient::new(api_base_url()).with_auth(token);
            let upload_req = CreateDocumentRequest {
                filename: file.name(),
                file_size: file.size() as u64,
                mime_type: file.type_(),
            };

            let result = async {
                let created = client
                    .create_document_upload(&notebook_id, &upload_req)
                    .await
                    .map_err(|error| {
                        format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "创建上传任务失败",
                                "Failed to create upload"
                            ),
                            error
                        )
                    })?;

                let bytes = browser_file_to_bytes(file).await.map_err(|error| {
                    format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "读取文件失败",
                            "Failed to read file bytes"
                        ),
                        error
                    )
                })?;

                HttpClient::new()
                    .put(&created.upload_url)
                    .body(bytes)
                    .send()
                    .await
                    .map_err(|error| {
                        format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "上传文件失败",
                                "Failed to upload file"
                            ),
                            error
                        )
                    })?
                    .error_for_status()
                    .map_err(|error| {
                        format!(
                            "{}: {}",
                            choose(locale.get_untracked(), "上传被拒绝", "Upload rejected"),
                            error
                        )
                    })?;

                client
                    .complete_upload(&created.document_id)
                    .await
                    .map_err(|error| {
                        format!(
                            "{}: {}",
                            choose(
                                locale.get_untracked(),
                                "完成上传失败",
                                "Failed to finalize upload"
                            ),
                            error
                        )
                    })?;

                Ok::<String, String>(created.document_id)
            }
            .await;

            match result {
                Ok(document_id) => {
                    set_selected_file.set(None);
                    set_selected_filename.set(String::new());
                    on_upload_success(document_id);
                }
                Err(message) => {
                    set_error.set(message);
                }
            }
            set_uploading.set(false);
        });
    };

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "上传文档", "Upload Document")}
            </h3>

            <div class="mb-4">
                <label class="app-form-label mb-2">
                    {move || choose(locale.get(), "选择文件", "Select File")}
                </label>
                <input
                    node_ref=input_ref
                    type="file"
                    accept=".pdf,.txt,.md,.doc,.docx"
                    on:change=handle_change
                    class="block w-full text-sm text-muted-foreground
                           file:mr-4 file:py-2 file:px-4
                           file:rounded file:border-0
                           file:text-sm file:font-semibold
                           file:bg-primary/10 file:text-primary
                           hover:file:bg-primary/15"
                />
                <p class="mt-1 text-xs text-muted-foreground">
                    {move || {
                        choose(
                            locale.get(),
                            "支持：PDF、TXT、MD、DOC、DOCX",
                            "Supported: PDF, TXT, MD, DOC, DOCX"
                        )
                    }}
                </p>
            </div>

            <Show when=move || !selected_filename.get().is_empty()>
                <div class="mb-4 rounded-xl border border-border bg-muted/60 px-3 py-2 text-sm text-foreground">
                    {selected_filename.get()}
                </div>
            </Show>

            <Show when=move || !error.get().is_empty()>
                <NoticeBanner message={error.get()} tone=NoticeTone::Danger />
            </Show>

            {/* Actions */}
            <div class="flex justify-end gap-3">
                <button
                    type="button"
                    class="app-button-ghost"
                    on:click=move |_| on_cancel_request()
                >
                    {move || choose(locale.get(), "取消", "Cancel")}
                </button>
                <button
                    type="button"
                    class="app-button-primary"
                    on:click=handle_upload
                    disabled=move || uploading.get()
                >
                    {move || {
                        if uploading.get() {
                            choose(locale.get(), "上传中...", "Uploading...")
                        } else {
                            choose(locale.get(), "上传", "Upload")
                        }
                    }}
                </button>
            </div>
        </div>
    }
    .into_any()
}

/// DocumentDetail component - displays document content and metadata
#[component]
pub fn DocumentDetail(
    source: SourceRow,
    on_close: impl Fn() + 'static,
    on_delete: impl Fn(String) + 'static,
    on_reindex: impl Fn(String) + 'static,
) -> impl IntoView {
    let auth = use_auth_state();
    let chat = use_chat_state();
    let locale = use_ui_prefs_state().locale;
    let workspace = use_workspace_state();

    let (content, set_content) = signal(String::new());
    let (parsed_preview, set_parsed_preview) = signal(Vec::<ParsedPreviewItem>::new());
    let (loading_content, set_loading_content) = signal(false);
    let (loading_more_preview, set_loading_more_preview) = signal(false);
    let (preview_has_more, set_preview_has_more) = signal(false);
    let (next_preview_cursor, set_next_preview_cursor) = signal(0_usize);
    let (content_error, set_content_error) = signal(String::new());
    let (loaded_content_key, set_loaded_content_key) = signal(String::new());

    // Clone source.id for use in closures
    let source_id_for_fetch = source.id.clone();
    let source_id_for_reindex = source.id.clone();
    let source_id_for_delete = source.id.clone();
    let source_id_for_scroll = source.id.clone();
    let source_id_for_focus_load = source.id.clone();
    let source_id_for_button_load = StoredValue::new(source.id.clone());

    // Fetch document content on mount
    let fetch_content = move || {
        let token = match auth.token.get() {
            Some(t) => t,
            None => return,
        };
        let locale_now = locale.get_untracked();

        set_loading_content.set(true);
        set_content_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token);
        let source_id = source_id_for_fetch.clone();

        spawn(async move {
            match client.get_parsed_preview(&source_id, 0, 120).await {
                Ok(resp) => {
                    let summary = resp.summary.unwrap_or_default();
                    set_content.set(summary);
                    set_parsed_preview.set(resp.items);
                    set_preview_has_more.set(resp.has_more);
                    set_next_preview_cursor.set(resp.next_cursor);
                }
                Err(_) => match client.get_document_content(&source_id).await {
                    Ok(resp) => {
                        set_content.set(resp.content);
                        set_parsed_preview.set(Vec::new());
                        set_preview_has_more.set(false);
                        set_next_preview_cursor.set(0);
                    }
                    Err(e) => {
                        set_content_error.set(format!(
                            "{}: {}",
                            choose(locale_now, "加载文档内容失败", "Failed to load content"),
                            e
                        ));
                    }
                },
            }
            set_loading_content.set(false);
        });
    };

    let auth_for_load = auth.clone();
    let source_id_for_load = source.id.clone();
    let fetch_content_on_mount = fetch_content.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|token| format!("{}:{}", token, source_id_for_load))
                .unwrap_or_default()
        },
        loaded_content_key,
        set_loaded_content_key,
        move || fetch_content_on_mount(),
    );

    Effect::new(move |_| {
        let focus = workspace.citation_focus.get();
        let items = parsed_preview.get();
        let has_more = preview_has_more.get();
        let loading_more = loading_more_preview.get();
        let Some(focus) = focus else {
            return;
        };
        if focus.doc_id != source_id_for_scroll {
            return;
        }
        if let Some(target) = items
            .iter()
            .find(|item| citation_focus_matches_item(&focus, item))
        {
            scroll_preview_item_into_view(&preview_item_dom_id(&source_id_for_scroll, target));
            return;
        }
        if has_more && !loading_more {
            append_preview_page(
                auth.token,
                locale,
                source_id_for_focus_load.clone(),
                content,
                set_content,
                loading_more_preview,
                set_loading_more_preview,
                next_preview_cursor,
                set_next_preview_cursor,
                preview_has_more,
                set_preview_has_more,
                set_parsed_preview,
                set_content_error,
            );
        }
    });

    view! {
        <div class="app-pane h-full">
            {/* Header */}
            <div class="app-pane-header">
                <div class="flex-1 min-w-0">
                    <h3 class="truncate text-lg font-semibold text-card-foreground">
                        {source.file_name.clone()}
                    </h3>
                    <div class="flex items-center gap-2 mt-1">
                        <span
                            class={format!(
                                "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium {}",
                                status_color(&source.status)
                            )}
                        >
                            {status_label(locale.get(), &source.status)}
                        </span>
                        <span class="text-xs text-muted-foreground">{source.title.clone()}</span>
                    </div>
                </div>
                <button
                    on:click=move |_| on_close()
                    class="app-button-ghost ml-4 !p-1 text-muted-foreground"
                    title={move || choose(locale.get(), "关闭文档详情", "Close document detail")}
                >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                    </svg>
                </button>
            </div>

            {/* Content area */}
            <div class="flex-1 overflow-y-auto p-4">
                <Show when=move || loading_content.get()>
                    <div class="flex items-center justify-center py-8">
                        <div class="text-muted-foreground">
                            {move || choose(locale.get(), "正在加载内容...", "Loading content...")}
                        </div>
                    </div>
                </Show>

                <Show when=move || !content_error.get().is_empty()>
                    <NoticeBanner message={content_error.get()} tone=NoticeTone::Danger />
                </Show>

                <Show when=move || !loading_content.get() && content_error.get().is_empty() && parsed_preview.get().is_empty()>
                    <pre class="whitespace-pre-wrap text-sm font-mono text-foreground">{content.get()}</pre>
                </Show>

                <Show when=move || !loading_content.get() && content_error.get().is_empty() && !parsed_preview.get().is_empty()>
                    <div class="space-y-3">
                        {parsed_preview.get().into_iter().map(|item| {
                            let item_id = preview_item_dom_id(&source.id, &item);
                            let active_focus = workspace
                                .citation_focus
                                .get()
                                .or_else(|| {
                                    chat.active_citation.get().map(|citation| CitationFocus::from_citation(&citation))
                                });
                            let is_active_citation = active_focus
                                .as_ref()
                                .map(|focus| focus.doc_id == source.id && citation_focus_matches_item(focus, &item))
                                .unwrap_or(false);
                            view! {
                                <div
                                    id={item_id}
                                    tabindex="-1"
                                    class="scroll-mt-4 rounded-xl border p-3 outline-none transition-colors"
                                    class=("border-primary/40", is_active_citation)
                                    class=("bg-primary/5", is_active_citation)
                                    class=("border-border", !is_active_citation)
                                    class=("bg-card", !is_active_citation)
                                >
                                    <div class="mb-1 text-xs text-muted-foreground">
                                        {move || choose(locale.get(), "页码 ", "Page ")}
                                        {item.page}
                                        {" · "}
                                        {move || choose(locale.get(), "游标 ", "Cursor ")}
                                        {item.cursor}
                                    </div>
                                    <div class="whitespace-pre-wrap text-sm text-foreground">{item.text}</div>
                                </div>
                            }
                        }).collect_view()}

                        <Show when=move || preview_has_more.get() || loading_more_preview.get()>
                            <div class="flex justify-center pt-2">
                                <button
                                    type="button"
                                    class="app-button-secondary"
                                    disabled=move || loading_more_preview.get()
                                    on:click=move |_| {
                                        append_preview_page(
                                            auth.token,
                                            locale,
                                            source_id_for_button_load.with_value(|id| id.clone()),
                                            content,
                                            set_content,
                                            loading_more_preview,
                                            set_loading_more_preview,
                                            next_preview_cursor,
                                            set_next_preview_cursor,
                                            preview_has_more,
                                            set_preview_has_more,
                                            set_parsed_preview,
                                            set_content_error,
                                        );
                                    }
                                >
                                    {move || {
                                        if loading_more_preview.get() {
                                            choose(locale.get(), "加载中...", "Loading...")
                                        } else {
                                            choose(locale.get(), "加载更多", "Load more")
                                        }
                                    }}
                                </button>
                            </div>
                        </Show>
                    </div>
                </Show>
            </div>

            {/* Actions */}
            <div class="flex items-center justify-end gap-2 border-t border-border bg-muted/40 px-4 py-3">
                <button
                    on:click=move |_| on_reindex(source_id_for_reindex.clone())
                    class="app-button-secondary"
                >
                    {move || choose(locale.get(), "重新索引", "Reindex")}
                </button>
                <button
                    on:click=move |_| on_delete(source_id_for_delete.clone())
                    class="app-button-danger"
                >
                    {move || choose(locale.get(), "删除", "Delete")}
                </button>
            </div>
        </div>
    }
}

/// DocumentListItem component - a single row in the sources list
#[component]
pub fn DocumentListItem(
    source: SourceRow,
    is_selected: bool,
    is_checked: bool,
    is_pinned: bool,
    checkbox_disabled: bool,
    on_click: impl Fn(SourceRow) + 'static,
    on_toggle_checked: impl Fn(String, bool) + 'static,
    on_toggle_pinned: impl Fn(String) + 'static,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let source_id = source.id.clone();
    let pin_source_id = source.id.clone();
    view! {
        <div
            class="cursor-pointer rounded-xl border px-3 py-2 text-sm transition-colors"
            class=("bg-primary/5", move || is_selected)
            class=("border-l-2", move || is_selected)
            class=("border-primary", move || is_selected)
            class=("border-border", move || !is_selected)
            class=("bg-card", move || !is_selected)
            class=("hover:bg-muted/60", move || !is_selected)
            on:click=move |_| on_click(source.clone())
        >
            <div class="flex items-center justify-between">
                <div class="flex items-start gap-2 flex-1 min-w-0">
                    <input
                        type="checkbox"
                        class="mt-0.5 h-4 w-4 rounded border-border text-primary focus:ring-primary disabled:cursor-not-allowed disabled:opacity-50"
                        prop:checked=move || is_checked
                        disabled=move || checkbox_disabled
                        on:click=move |ev| ev.stop_propagation()
                        on:change=move |ev| {
                            on_toggle_checked(source_id.clone(), event_target_checked(&ev));
                        }
                    />
                    <div class="flex-1 min-w-0">
                        <p class="truncate font-medium text-foreground">{source.file_name.clone()}</p>
                        <p class="mt-0.5 text-xs text-muted-foreground">{source.title.clone()}</p>
                    </div>
                </div>
                <div class="ml-2 flex shrink-0 items-center gap-2">
                    <button
                        type="button"
                        class="rounded-lg p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                        on:click=move |ev| {
                            ev.stop_propagation();
                            on_toggle_pinned(pin_source_id.clone());
                        }
                        title={move || {
                            if is_pinned {
                                choose(locale.get(), "取消固定", "Unpin")
                            } else {
                                choose(locale.get(), "固定到顶部", "Pin to top")
                            }
                        }}
                    >
                        <svg class="h-4 w-4" fill={if is_pinned { "currentColor" } else { "none" }} stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.7" d="M16 7l-1.5 6 3.5 3.5-1.5 1.5-3.5-3.5L7 16l4-9 5-0zM8 21l4-4"/>
                        </svg>
                    </button>
                    <span
                        class={format!(
                            "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium {}",
                            status_color(&source.status)
                        )}
                    >
                        {status_label(locale.get(), &source.status)}
                    </span>
                </div>
            </div>
        </div>
    }
}
