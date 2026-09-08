use crate::i18n::use_i18n;
use leptos::prelude::*;
use web_sdk::{Capability, mode_line, toggle_capability};

#[component]
pub fn ScopeBar(
    capabilities: RwSignal<Vec<Capability>>,
    capabilities_manual: RwSignal<bool>,
    ready_count: Signal<usize>,
    disabled: Signal<bool>,
    knowledge_chat: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let rag_available = Signal::derive(move || ready_count.get() > 0);
    let toggle = move |cap: Capability| {
        if disabled.get() {
            return;
        }
        if cap == Capability::Rag && ready_count.get() == 0 {
            return;
        }
        capabilities.update(|current| *current = toggle_capability(current, cap));
        capabilities_manual.set(true);
    };

    view! {
        <div
            class="chat-scope-bar"
            data-testid="scope-bar"
            role="group"
            aria-label=move || i18n.t("workspaceChatCapabilityLabel")
        >
            <div class="chat-scope-chips">
                <Show when=move || knowledge_chat>
                <button
                    type="button"
                    class=move || chip_class(capabilities.get().contains(&Capability::Rag))
                    data-testid="scope-cap-rag"
                    aria-pressed=move || {
                        if capabilities.get().contains(&Capability::Rag) {
                            "true"
                        } else {
                            "false"
                        }
                    }
                    disabled=move || disabled.get() || !rag_available.get()
                    on:click=move |_| toggle(Capability::Rag)
                >
                    {move || i18n.t("workspaceChatCapRag")}
                    <Show when=move || {
                        capabilities.get().contains(&Capability::Rag) && ready_count.get() > 0
                    }>
                        <span class="chat-scope-badge" aria-hidden="true">
                            {move || ready_count.get().to_string()}
                        </span>
                    </Show>
                </button>
                </Show>
                <button
                    type="button"
                    class=move || chip_class(capabilities.get().contains(&Capability::Search))
                    data-testid="scope-cap-search"
                    aria-pressed=move || {
                        if capabilities.get().contains(&Capability::Search) {
                            "true"
                        } else {
                            "false"
                        }
                    }
                    disabled=move || disabled.get()
                    on:click=move |_| toggle(Capability::Search)
                >
                    {move || i18n.t("workspaceChatCapSearch")}
                </button>
            </div>
            <p class="chat-scope-mode" data-testid="scope-mode-line">
                {move || i18n.t(if !knowledge_chat {
                    if capabilities.get().contains(&Capability::Search) { "chat.personalSearch" } else { "chat.personalDirect" }
                } else if !rag_available.get() { "chat.workspaceEmpty" }
                else { mode_line(&capabilities.get(), true) })}
            </p>
        </div>
    }
}

fn chip_class(pressed: bool) -> &'static str {
    if pressed {
        "chat-scope-chip is-pressed"
    } else {
        "chat-scope-chip"
    }
}
