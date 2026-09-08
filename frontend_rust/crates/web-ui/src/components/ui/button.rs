// Adapted from Rust/UI; provenance and MIT license: frontend_rust/THIRD_PARTY.md.
use leptos::prelude::*;

#[derive(Clone, Copy, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
}

#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(default = "button")] button_type: &'static str,
    #[prop(default = Signal::derive(|| false))] disabled: Signal<bool>,
    #[prop(optional)] test_id: &'static str,
    #[prop(optional)] on_click: Option<Callback<()>>,
    children: Children,
) -> impl IntoView {
    let class = match variant {
        ButtonVariant::Primary => "ui-button ui-button-primary",
        ButtonVariant::Secondary => "ui-button ui-button-secondary",
    };
    view! {
        <button
            type=button_type
            class=class
            disabled=move || disabled.get()
            data-testid=test_id
            on:click=move |_| {
                if !disabled.get_untracked() {
                    if let Some(callback) = on_click {
                        callback.run(());
                    }
                }
            }
        >
            {children()}
        </button>
    }
}
