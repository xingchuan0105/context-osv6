use crate::api_base::poc_api_base;
use crate::i18n::use_i18n;
use leptos::prelude::*;
use web_sdk::{read_browser_auth, BrowserRestClient, NotificationRow};

#[component]
pub fn NotificationBell() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    let trigger = NodeRef::<leptos::html::Button>::new();
    let items = RwSignal::new(Vec::<NotificationRow>::new());
    let error = RwSignal::new(None::<String>);
    let loading = RwSignal::new(false);

    let resolve_token = move || {
        let tok = token.get();
        if !tok.is_empty() {
            return tok;
        }
        read_browser_auth()
            .map(|auth| {
                token.set(auth.token.clone());
                auth.token
            })
            .unwrap_or_default()
    };

    let reload = move || {
        let tok = resolve_token();
        if tok.is_empty() {
            items.set(Vec::new());
            return;
        }
        if loading.get_untracked() { return; }
        error.set(None);
        loading.set(true);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.list_notifications().await {
                Ok(resp) => items.set(resp.notifications),
                Err(err) => error.set(Some(err.to_string())),
            }
            loading.set(false);
        });
    };

    let on_open = move |_| {
        let next = !open.get();
        open.set(next);
        if next {
            reload();
        }
    };

    view! {
        <div class="app-menu" on:keydown=move |event: leptos::ev::KeyboardEvent| {
            if event.key() == "Escape" && open.get_untracked() {
                event.prevent_default(); open.set(false);
                #[cfg(target_arch = "wasm32")]
                if let Some(button) = trigger.get_untracked() { let _ = button.focus(); }
            }
        }>
            <button
                type="button"
                class="app-top-bar-capsule"
                node_ref=trigger
                aria-expanded=move || open.get()
                aria-label=move || i18n.t("notifications.trigger")
                data-testid="notification-bell"
                on:click=on_open
            >
                {move || i18n.t("notifications.trigger")}
                {move || {
                    let unread = items
                        .get()
                        .iter()
                        .filter(|row| row.read_at.is_none())
                        .count();
                    (unread > 0).then(|| {
                        view! {
                            <span class="app-notify-badge" data-testid="notification-bell-unread">
                                {if unread > 99 { "99+".to_string() } else { unread.to_string() }}
                            </span>
                        }
                    })
                }}
            </button>
            <Show when=move || open.get()>
                <button
                    type="button"
                    class="app-menu-dismiss"
                    tabindex="-1"
                    aria-label=move || i18n.t("notifications.close")
                    on:click=move |_| open.set(false)
                />
                <div
                    class="app-menu-panel app-notify-panel"
                    role="region"
                    aria-label=move || i18n.t("notifications.listLabel")
                    data-testid="notification-bell-panel"
                >
                    {move || {
                        if loading.get() {
                            view! { <p class="app-notify-empty">{i18n.t("notifications.loading")}</p> }.into_any()
                        } else if let Some(message) = error.get() {
                            view! { <div data-testid="notification-error"><p role="alert">{message}</p><button type="button" on:click=move |_| reload()>{move || i18n.t("common.retry")}</button></div> }.into_any()
                        } else if items.get().is_empty() {
                            view! {
                                <div class="app-notify-empty" data-testid="notification-empty">
                                    <strong>{i18n.t("notifications.emptyTitle")}</strong>
                                    <p>{i18n.t("notifications.emptyBody")}</p>
                                </div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <ul class="app-notify-list">
                                    <For
                                        each=move || items.get()
                                        key=|row| (row.id.clone(), row.read_at.clone())
                                        children=move |row| {
                                            let id = row.id.clone();
                                            let unread = row.read_at.is_none();
                                            view! {
                                                <li
                                                    class=if unread {
                                                        "app-notify-item is-unread"
                                                    } else {
                                                        "app-notify-item"
                                                    }
                                                    data-testid="notification-item"
                                                >
                                                    <button
                                                        type="button"
                                                        class="app-notify-item-btn"
                                                        on:click=move |_| {
                                                            let tok = if token.get_untracked().is_empty() {
                                                                read_browser_auth().map(|a| a.token).unwrap_or_default()
                                                            } else {
                                                                token.get_untracked()
                                                            };
                                                            if tok.is_empty() {
                                                                return;
                                                            }
                                                            let id = id.clone();
                                                            leptos::task::spawn_local(async move {
                                                                let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                                                                match client.mark_notification_read(&id).await {
                                                                    Ok(()) => {
                                                                    items.update(|list| {
                                                                        if let Some(row) = list.iter_mut().find(|row| row.id == id) {
                                                                            row.read_at = Some("read".to_string());
                                                                        }
                                                                    });
                                                                    }
                                                                    Err(err) => error.set(Some(err.to_string())),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        <strong>{row.title.clone()}</strong>
                                                        <span>{row.body.clone()}</span>
                                                    </button>
                                                </li>
                                            }
                                        }
                                    />
                                </ul>
                            }
                            .into_any()
                        }
                    }}
                </div>
            </Show>
        </div>
    }
}
