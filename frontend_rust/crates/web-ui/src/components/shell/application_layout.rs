use super::{ContextTopBar, NavigationRail, NavigationState};
use crate::api_base::poc_api_base;
use crate::i18n::{t_now, use_i18n};
use crate::routes::dest;
use contracts::workspaces::ChatSession;
use leptos::prelude::*;
use web_sdk::BrowserRestClient;

#[component]
pub fn ApplicationLayout(
    #[prop(default = Signal::derive(|| t_now("chat.pageTitle")))] title: Signal<String>,
    #[prop(optional)] sidebar: Option<Children>,
    #[prop(optional)] actions: Option<Children>,
    children: Children,
) -> impl IntoView {
    let navigation = NavigationState::new();
    let location = leptos_router::hooks::use_location();
    Effect::new(move |_| {
        let _ = (location.pathname.get(), location.search.get());
        navigation.open.set(false);
    });
    view! {
        <div class="app-layout application-layout"
            data-navigation-open=move || navigation.open.get().to_string()
            data-navigation-collapsed=move || navigation.collapsed.get().to_string()>
            <NavigationRail state=navigation>
                {match sidebar { Some(render) => render(), None => view! { <GlobalNavigation/> }.into_any() }}
            </NavigationRail>
            <div class="app-layout-main" inert=move || navigation.open.get()>
                <ContextTopBar state=navigation title=title>{actions.map(|render| render())}</ContextTopBar>
                <div class="application-page">{children()}</div>
            </div>
        </div>
    }
}

#[component]
fn GlobalNavigation() -> impl IntoView {
    let i18n = use_i18n();
    let token = expect_context::<RwSignal<String>>();
    let sessions = RwSignal::new(Vec::<ChatSession>::new());
    let error = RwSignal::new(false);
    let refresh = RwSignal::new(0_u64);
    Effect::new(move |_| {
        let current = token.get();
        let generation = refresh.get();
        if current.is_empty() { sessions.set(Vec::new()); return; }
        error.set(false);
        leptos::task::spawn_local(async move {
            let result = BrowserRestClient::new(&poc_api_base(), Some(current.clone())).list_sessions().await;
            if token.try_get_untracked().as_deref() != Some(current.as_str()) || refresh.try_get_untracked() != Some(generation) { return; }
            match result { Ok(response) => sessions.set(response.sessions), Err(_) => error.set(true) }
        });
    });
    view! {
        <nav class="application-destinations">
            <a href=dest::CHAT>{move || i18n.t("navigation.chat")}</a>
            <a href=dest::DASHBOARD data-testid="all-workspaces-link">{move || i18n.t("navigation.workspace")}</a>
        </nav>
        <p class="chat-sessions-hint">{move || i18n.t("navigation.history")}</p>
        <Show when=move || error.get()>
            <button type="button" on:click=move |_| refresh.update(|n| *n += 1)>{move || i18n.t("chat.retry")}</button>
        </Show>
        <ul class="chat-session-items">
            <For each=move || sessions.get() key=|session| session.id.clone() children=move |session| {
                let href = session.workspace_id.as_ref().map(|wid| format!("/dashboard/{wid}?session={}", session.id)).unwrap_or_else(|| format!("/chat/{}", session.id));
                let title = session.title.unwrap_or_else(|| i18n.t("chat.untitled"));
                view! { <li><a class="application-session" href=href>{title}{session.workspace_name.map(|name| view! { <small>{name}</small> })}</a></li> }
            }/>
        </ul>
    }
}
