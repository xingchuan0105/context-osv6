use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct Toaster {
    pub message: RwSignal<Option<String>>,
}

impl Toaster {
    pub fn push(self, text: impl Into<String>) {
        self.message.set(Some(text.into()));
    }
}

pub fn provide_toaster() -> Toaster {
    let toaster = Toaster {
        message: RwSignal::new(None::<String>),
    };
    provide_context(toaster);
    toaster
}

#[component]
pub fn ToastHost() -> impl IntoView {
    let toaster = expect_context::<Toaster>();
    Effect::new(move |_| {
        if toaster.message.get().is_none() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(handle) = set_timeout_with_handle(
                move || toaster.message.set(None),
                std::time::Duration::from_secs(3),
            ) {
                on_cleanup(move || handle.clear());
            }
        }
    });
    view! {
        <div
            class="app-toast"
            role="status"
            data-testid="app-toast"
            hidden=move || toaster.message.get().is_none()
        >
            {move || toaster.message.get().unwrap_or_default()}
        </div>
    }
}
