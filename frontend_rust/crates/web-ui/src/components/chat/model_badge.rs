use leptos::prelude::*;
use web_sdk::model_role_label;

#[component]
pub fn ModelRoleBadge(
    model_role: Signal<String>,
    has_byok: Signal<bool>,
) -> impl IntoView {
    view! {
        <div
            class="chat-model-badge"
            data-testid="model-role-badge"
            role="status"
            aria-label="当前模型配置"
        >
            <span class="chat-model-label">
                {move || model_role_label(&model_role.get(), has_byok.get())}
            </span>
        </div>
    }
}
