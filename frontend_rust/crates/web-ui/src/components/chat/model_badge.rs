use crate::i18n::use_i18n;
use leptos::prelude::*;

#[component]
pub fn ModelRoleBadge(
    model_role: Signal<String>,
    has_byok: Signal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div
            class="chat-model-badge"
            data-testid="model-role-badge"
            role="status"
            aria-label=move || i18n.t("chat.modelBadgeLabel")
        >
            <span class="chat-model-label">
                {move || {
                    match model_role.get().as_str() {
                        "quick_chat" if has_byok.get() => i18n.t("chat.modelRole.quickChatByok"),
                        "quick_chat" => i18n.t("chat.modelRole.quickChatDefault"),
                        "agent" => i18n.t("chat.modelRole.agent"),
                        other => other.to_string(),
                    }
                }}
            </span>
        </div>
    }
}
