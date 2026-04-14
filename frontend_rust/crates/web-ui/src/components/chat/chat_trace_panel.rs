//! Chat trace panel - displays debug/trace information

use leptos::prelude::*;

use crate::i18n::{Locale, choose};
use crate::state::chat::{AgentMode, ChatStatus, use_chat_state};
use crate::state::ui_prefs::use_ui_prefs_state;
use crate::state::workspace::use_workspace_state;

fn chat_status_label(locale: Locale, status: ChatStatus) -> &'static str {
    match status {
        ChatStatus::Idle => choose(locale, "就绪", "Idle"),
        ChatStatus::Submitting => choose(locale, "发送中", "Submitting"),
        ChatStatus::Streaming => choose(locale, "生成中", "Streaming"),
        ChatStatus::Done => choose(locale, "已完成", "Done"),
        ChatStatus::Error => choose(locale, "出错", "Error"),
    }
}

fn agent_mode_label(locale: Locale, mode: AgentMode) -> &'static str {
    match mode {
        AgentMode::Rag => choose(locale, "知识库", "RAG"),
        AgentMode::Search => choose(locale, "检索", "Search"),
        AgentMode::General => choose(locale, "通用", "General"),
    }
}

/// ChatTracePanel component for the right sidebar
#[component]
pub fn ChatTracePanel() -> impl IntoView {
    let chat = use_chat_state();
    let locale = use_ui_prefs_state().locale;

    view! {
        <div class="space-y-4">
            <h3 class="text-sm font-medium text-foreground">
                {move || choose(locale.get(), "调试信息", "Trace Information")}
            </h3>

            {/* Status display */}
            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "状态", "Status")}
                </div>
                <div class="text-sm font-medium">
                    {move || chat_status_label(locale.get(), chat.status.get())}
                </div>
            </div>

            {/* Agent mode display */}
            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "代理模式", "Agent Mode")}
                </div>
                <div class="text-sm font-medium">
                    {move || agent_mode_label(locale.get(), chat.agent_mode.get())}
                </div>
            </div>

            <Show when={move || chat.planner_mode.get().is_some()}>
                <div class="app-inline-surface">
                    <div class="mb-1 text-xs text-muted-foreground">
                        {move || choose(locale.get(), "规划模式", "Planner Mode")}
                    </div>
                    <div class="text-sm font-medium">{chat.planner_mode.get().unwrap_or_default()}</div>
                </div>
            </Show>

            {/* Message count */}
            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "消息数", "Messages")}
                </div>
                <div class="text-sm font-medium">{chat.messages.get().len()}</div>
            </div>

            {/* Current answer preview */}
            <Show when={move || !chat.current_answer.get().is_empty()}>
                <div class="app-inline-surface">
                    <div class="mb-1 text-xs text-muted-foreground">
                        {move || choose(locale.get(), "当前回答（片段）", "Current Answer (partial)")}
                    </div>
                    <div class="max-h-32 overflow-y-auto text-sm text-foreground">
                        {chat.current_answer.get()}
                    </div>
                </div>
            </Show>

            {/* Citations count */}
            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "引用数", "Citations")}
                </div>
                <div class="text-sm font-medium">{chat.citations.get().len()}</div>
            </div>

            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "资料引用数", "Source References")}
                </div>
                <div class="text-sm font-medium">{chat.source_count.get()}</div>
            </div>

            <Show when={move || !chat.trace_events.get().is_empty()}>
                <div class="app-inline-surface">
                    <div class="mb-2 text-xs text-muted-foreground">
                        {move || choose(locale.get(), "事件", "Events")}
                    </div>
                    <div class="space-y-1">
                        {chat.trace_events.get().into_iter().map(|entry| {
                            view! { <div class="text-xs text-foreground">{entry}</div> }
                        }).collect_view()}
                    </div>
                </div>
            </Show>

            <Show when={move || chat.rag_trace_json.get().is_some()}>
                <details class="group bg-card rounded-lg border border-border shadow-sm overflow-hidden transition-all duration-200">
                    <summary class="flex items-center justify-between p-3 cursor-pointer select-none bg-muted/30 hover:bg-muted/50 transition-colors">
                        <span class="text-xs font-semibold text-foreground tracking-wide uppercase">
                            {move || choose(locale.get(), "RAG 轨迹可视化", "RAG Trace Visualization")}
                        </span>
                        <svg class="w-4 h-4 text-muted-foreground transition-transform duration-200 group-open:rotate-180" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                        </svg>
                    </summary>
                    <div class="p-3 border-t border-border bg-card">
                        <pre class="text-[10px] text-muted-foreground whitespace-pre-wrap break-words overflow-x-auto font-mono">
                            {chat.rag_trace_json.get().unwrap_or_default()}
                        </pre>
                    </div>
                </details>
            </Show>

            {/* Error message */}
            <Show when={move || chat.error_message.get().is_some()}>
                <div class="p-3 bg-red-50 rounded-lg border border-red-200">
                    <div class="text-xs text-red-500 mb-1">
                        {move || choose(locale.get(), "错误", "Error")}
                    </div>
                    <div class="text-sm text-red-700">{chat.error_message.get().unwrap_or_default()}</div>
                </div>
            </Show>
        </div>
    }
}

/// Evidence panel - displays citation details
#[component]
pub fn EvidencePanel() -> impl IntoView {
    let chat = use_chat_state();
    let workspace = use_workspace_state();
    let locale = use_ui_prefs_state().locale;

    view! {
        <div class="space-y-3">
            <h3 class="text-sm font-medium text-foreground">
                {move || choose(locale.get(), "引用定位", "Citations")}
            </h3>

            <Show when={move || chat.citations.get().is_empty()}>
                <p class="text-sm text-muted-foreground">
                    {move || choose(locale.get(), "当前还没有引用", "No citations yet")}
                </p>
            </Show>

            <Show when={move || !chat.citations.get().is_empty()}>
                <div class="space-y-2">
                    {chat.citations.get().into_iter().enumerate().map(|(idx, citation)| {
                        let citation_id = citation.citation_id;
                        let is_active = chat.active_citation.get().map(|c| c.citation_id == citation_id).unwrap_or(false);
                        let citation_clone = citation.clone();
                        let chat_clone = chat.clone();
                        let workspace = workspace.clone();
                        let idx_display = idx + 1;
                        view! {
                            <button
                                class="w-full text-left p-3 rounded-lg border transition-all duration-200 transform hover:-translate-y-0.5"
                                class=("border-primary/50 shadow-md", is_active)
                                class=("bg-primary/5", is_active)
                                class=("border-border shadow-sm", !is_active)
                                class=("bg-card", !is_active)
                                class=("hover:bg-muted/50 hover:border-border", !is_active)
                                on:click=move |_| {
                                    chat_clone.set_active_citation.set(Some(citation_clone.clone()));
                                    workspace.request_citation_focus(&citation_clone);
                                }
                            >
                                <div class="flex items-start gap-3">
                                    <span class="inline-flex items-center justify-center w-6 h-6 rounded border border-border bg-card/80 text-primary text-xs font-semibold shrink-0 shadow-sm">
                                        {idx_display}
                                    </span>
                                    <div class="flex-1 min-w-0">
                                        <p class="truncate text-sm font-medium text-foreground">
                                            {citation.doc_name.clone()}
                                        </p>
                                        {citation.preview.as_ref().map(|p| {
                                            view! {
                                                <p class="mt-1 line-clamp-2 text-xs text-muted-foreground">{p.clone()}</p>
                                            }
                                        })}
                                        {citation.image_url.as_ref().map(|image_url| {
                                            view! {
                                                <img
                                                    src=image_url.clone()
                                                    alt=citation.caption.clone().unwrap_or_else(|| citation.doc_name.clone())
                                                    class="mt-2 max-h-40 w-auto max-w-full rounded-lg border border-border bg-muted object-contain"
                                                />
                                            }
                                        })}
                                        {citation.caption.as_ref().map(|caption| {
                                            view! {
                                                <p class="mt-1 line-clamp-2 text-xs text-muted-foreground">{caption.clone()}</p>
                                            }
                                        })}
                                        {citation.content.as_ref().map(|c| {
                                            view! {
                                                <p class="mt-1 line-clamp-3 text-xs text-muted-foreground">{c.clone()}</p>
                                            }
                                        })}
                                        <div class="flex items-center gap-2 mt-1">
                                            <span class="text-xs text-muted-foreground">
                                                {move || choose(locale.get(), "得分", "Score")}
                                                {": "}
                                                {format!("{:.2}", citation.score)}
                                            </span>
                                            {citation.layer.as_ref().map(|layer| {
                                                view! {
                                                    <span class="text-xs text-muted-foreground">
                                                        {move || choose(locale.get(), "层级", "Layer")}
                                                        {": "}
                                                        {layer.clone()}
                                                    </span>
                                                }
                                            })}
                                        </div>
                                    </div>
                                </div>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}

/// Session panel - displays session information
#[component]
pub fn SessionPanel() -> impl IntoView {
    let chat = use_chat_state();
    let locale = use_ui_prefs_state().locale;

    view! {
        <div class="space-y-4">
            <h3 class="text-sm font-medium text-foreground">
                {move || choose(locale.get(), "会话信息", "Session Information")}
            </h3>

            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "会话 ID", "Session ID")}
                </div>
                <div class="text-sm font-medium break-all">
                    {move || {
                        chat.session_id
                            .get()
                            .unwrap_or_else(|| choose(locale.get(), "未开始", "Not started").to_string())
                    }}
                </div>
            </div>

            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "会话消息数", "Messages in session")}
                </div>
                <div class="text-lg font-semibold">{chat.messages.get().len()}</div>
            </div>

            <div class="app-inline-surface">
                <div class="mb-1 text-xs text-muted-foreground">
                    {move || choose(locale.get(), "当前模式", "Current mode")}
                </div>
                <div class="text-sm font-medium">
                    {move || agent_mode_label(locale.get(), chat.agent_mode.get())}
                </div>
            </div>

            <button
                class="app-button-danger w-full"
                on:click=move |_| {
                    chat.reset();
                }
            >
                {move || choose(locale.get(), "清空对话", "Clear Conversation")}
            </button>
        </div>
    }
}
