use leptos::prelude::*;
use web_sdk::{Capability, mode_line, toggle_capability};

#[component]
pub fn ScopeBar(
    capabilities: RwSignal<Vec<Capability>>,
    capabilities_manual: RwSignal<bool>,
    ready_count: Signal<usize>,
    disabled: Signal<bool>,
) -> impl IntoView {
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
            aria-label="能力标签"
        >
            <div class="chat-scope-chips">
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
                    disabled=move || disabled.get()
                    on:click=move |_| toggle(Capability::Rag)
                >
                    "知识库"
                    <Show when=move || {
                        capabilities.get().contains(&Capability::Rag) && ready_count.get() > 0
                    }>
                        <span class="chat-scope-badge" aria-hidden="true">
                            {move || ready_count.get().to_string()}
                        </span>
                    </Show>
                </button>
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
                    "网络搜索"
                </button>
            </div>
            <p class="chat-scope-mode" data-testid="scope-mode-line">
                {move || mode_line(&capabilities.get(), rag_available.get())}
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
