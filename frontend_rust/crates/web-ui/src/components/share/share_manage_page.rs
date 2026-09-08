use crate::api_base::poc_api_base;
use crate::components::shell::ProductChrome;
use contracts::share::{AccessLogEntry, ShareAnalyticsResponse, ShareSettings};
use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use web_sdk::BrowserRestClient;

#[derive(Params, PartialEq, Clone, Debug)]
struct ShareParams {
    workspace_id: Option<String>,
}

#[component]
pub fn WorkspaceSharePage() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ShareParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let share_settings = RwSignal::new(None::<ShareSettings>);
    let current_token = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let copied = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let origin = RwSignal::new(String::new());
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        if let Some(window) = web_sys::window() {
            if let Ok(value) = window.location().origin() { origin.set(value); }
        }
        let _ = current_token.get();
        copied.set(false);
    });

    let loading = RwSignal::new(true);
    let retry = RwSignal::new(0_u64);
    let load_sequence = RwSignal::new(0_u64);
    Effect::new(move |_| {
        let _ = retry.get();
        load_sequence.update(|n| *n += 1);
        let generation = load_sequence.get_untracked();
        loading.set(true); error.set(None);
        current_token.set(None); share_settings.set(None); busy.set(false);
        let wid = workspace_id.get();
        if wid.is_empty() {
            return;
        }
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() { loading.set(false); error.set(Some(i18n.t("pricing.loginRequired"))); return; }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = client.get_share_settings(&wid).await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str()) || load_sequence.try_get_untracked() != Some(generation) { return; }
            match result {
                Ok(settings) => {
                    current_token.set((!settings.share_token.is_empty()).then(|| settings.share_token.clone()));
                    share_settings.set(Some(settings));
                }
                Err(err) => error.set(Some(err.to_string())),
            }
            loading.set(false);
        });
    });

    let on_create_share = move |_| {
        let wid = workspace_id.get_untracked();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() || busy.get() {
            return;
        }

        busy.set(true);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = client.create_share(&wid).await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str()) { return; }
            match result {
                Ok(resp) => {
                    current_token.set(Some(resp.share_token));
                }
                Err(err) => {
                    error.set(Some(err.to_string()));
                }
            }
            busy.set(false);
        });
    };

    let on_revoke_share = move |_| {
        let wid = workspace_id.get_untracked();
        let Some(stok) = current_token.get_untracked() else {
            return;
        };
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() || busy.get() {
            return;
        }

        busy.set(true);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = client.revoke_share(&wid, &stok).await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str()) { return; }
            match result { Ok(()) => { current_token.set(None); share_settings.set(None); }, Err(err) => error.set(Some(err.to_string())) }
            busy.set(false);
        });
    };

    let on_copy_link = move |_| {
        let Some(tok) = current_token.get_untracked() else {
            return;
        };
        let _link = format!("{}/shared/kb/{tok}", origin.get_untracked());
        copied.set(false);
        error.set(None);
        #[cfg(target_arch = "wasm32")]
        if let Some(w) = web_sys::window() {
            let promise = w.navigator().clipboard().write_text(&_link);
            leptos::task::spawn_local(async move {
                let result = wasm_bindgen_futures::JsFuture::from(promise).await;
                if current_token.get_untracked().as_deref() != Some(tok.as_str()) { return; }
                match result {
                    Ok(_) => copied.set(true),
                    Err(_) => error.set(Some(i18n.t("share.copyFailed"))),
                }
            });
        }
    };

    view! {
        <ProductChrome>
        <div class="settings-shell" data-testid="workspace-share-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href=move || format!("/dashboard/{}", workspace_id.get()) class="settings-back-link">
                        {move || i18n.t("share.backWorkbench")}
                    </a>
                    <h1 class="settings-title">{move || i18n.t("share.centerTitle")}</h1>
                </div>
            </header>

            <nav class="settings-nav" aria-label={move || i18n.t("share.navLabel")}>
                <a href=move || format!("/dashboard/{}/share", workspace_id.get()) class="settings-nav-item is-active">
                    {move || i18n.t("share.linkSettings")}
                </a>
                <a href=move || format!("/dashboard/{}/share/access-logs", workspace_id.get()) class="settings-nav-item">
                    {move || i18n.t("share.logsTitle")}
                </a>
                <a href=move || format!("/dashboard/{}/share/analytics", workspace_id.get()) class="settings-nav-item">
                    {move || i18n.t("share.analyticsNav")}
                </a>
            </nav>

            <main class="settings-content">
                <p role="status" hidden=move || !loading.get()>{move || i18n.t("common.loading")}</p>
                <div hidden=move || error.get().is_none()><p role="alert" data-testid="share-error">{move || error.get()}</p><button type="button" on:click=move |_| retry.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button></div>
                <section class="settings-panel" data-testid="share-panel">
                    <h2>{move || i18n.t("share.readOnly")}</h2>
                    <p class="settings-panel-desc">
                        {move || i18n.t("share.readOnlyBody")}
                    </p>
                    {move || {
                        if let Some(tok) = current_token.get() {
                            view! {
                                <div class="share-active-box" data-testid="share-active-box">
                                    <div class="share-link-row">
                                        <span class="share-link-text" data-testid="share-url">{format!("{}/shared/kb/{tok}", origin.get())}</span>
                                        <button
                                            type="button"
                                            class="dashboard-btn-confirm"
                                            data-testid="copy-share-btn"
                                            on:click=on_copy_link
                                        >
                                            {move || i18n.t(if copied.get() { "share.copied" } else { "share.copy" })}
                                        </button>
                                    </div>
                                    <button
                                        type="button"
                                        class="settings-btn-revoke"
                                        data-testid="revoke-share-btn"
                                        disabled=move || busy.get() || loading.get()
                                        on:click=on_revoke_share
                                    >
                                        {move || i18n.t("share.revoke")}
                                    </button>
                                </div>
                            }
                            .into_any()
                        } else {
                            view! {
                                <div class="share-inactive-box">
                                    <button
                                        type="button"
                                        class="dashboard-create-btn"
                                        data-testid="create-share-btn"
                                        disabled=move || busy.get()
                                        on:click=on_create_share
                                    >
                                        {move || i18n.t("share.create")}
                                    </button>
                                </div>
                            }
                            .into_any()
                        }
                    }}
                </section>
            </main>
        </div>
        </ProductChrome>
    }
}

#[component]
pub fn WorkspaceShareLogsPage() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let error = RwSignal::new(None::<String>);
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ShareParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let logs = RwSignal::new(Vec::<AccessLogEntry>::new());

    let loading = RwSignal::new(true);
    let retry = RwSignal::new(0_u64);
    let load_sequence = RwSignal::new(0_u64);
    Effect::new(move |_| {
        let _ = retry.get();
        load_sequence.update(|n| *n += 1);
        let generation = load_sequence.get_untracked();
        loading.set(true); error.set(None);
        let wid = workspace_id.get();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() { loading.set(false); error.set(Some(i18n.t("pricing.loginRequired"))); return; }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = client.get_share_access_logs(&wid).await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str()) || load_sequence.try_get_untracked() != Some(generation) { return; }
            match result { Ok(resp) => logs.set(resp.logs), Err(err) => error.set(Some(err.to_string())) }
            loading.set(false);
        });
    });

    view! {
        <ProductChrome>
        <div class="settings-shell" data-testid="share-logs-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href=move || format!("/dashboard/{}/share", workspace_id.get()) class="settings-back-link">
                        {move || i18n.t("share.backCenter")}
                    </a>
                    <h1 class="settings-title">{move || i18n.t("share.logsTitle")}</h1>
                </div>
            </header>
            <main class="settings-content">
                <p role="status" hidden=move || !loading.get()>{move || i18n.t("common.loading")}</p>
                <div hidden=move || error.get().is_none()><p role="alert">{move || error.get()}</p><button type="button" on:click=move |_| retry.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button></div>
                <section class="settings-panel">
                    <p hidden=move || loading.get() || error.get().is_some() || !logs.get().is_empty()>{move || i18n.t("share.logsEmpty")}</p>
                    <ul class="share-log-list" hidden=move || loading.get() || error.get().is_some()>
                        <For
                            each=move || logs.get()
                            key=|log| log.id.clone()
                            children=move |log| {
                                view! {
                                    <li class="share-log-item">
                                        <span>{log.accessed_at}</span>
                                        <span>{log.action}</span>
                                        <span>{i18n.tf("share.visitor", &[("id", log.visitor_id.as_str())])}</span>
                                    </li>
                                }
                            }
                        />
                    </ul>
                </section>
            </main>
        </div>
        </ProductChrome>
    }
}

#[component]
pub fn WorkspaceShareAnalyticsPage() -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let error = RwSignal::new(None::<String>);
    let token = expect_context::<RwSignal<String>>();
    let params = use_params::<ShareParams>();
    let workspace_id = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.workspace_id.clone())
            .unwrap_or_default()
    });

    let analytics = RwSignal::new(None::<ShareAnalyticsResponse>);

    let loading = RwSignal::new(true);
    let retry = RwSignal::new(0_u64);
    let load_sequence = RwSignal::new(0_u64);
    Effect::new(move |_| {
        let _ = retry.get();
        load_sequence.update(|n| *n += 1);
        let generation = load_sequence.get_untracked();
        loading.set(true); error.set(None);
        let wid = workspace_id.get();
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if wid.is_empty() || tok.is_empty() { loading.set(false); error.set(Some(i18n.t("pricing.loginRequired"))); return; }

        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            let result = client.get_share_analytics(&wid).await;
            if workspace_id.try_get_untracked().as_deref() != Some(wid.as_str()) || load_sequence.try_get_untracked() != Some(generation) { return; }
            match result { Ok(resp) => analytics.set(Some(resp)), Err(err) => error.set(Some(err.to_string())) }
            loading.set(false);
        });
    });

    view! {
        <ProductChrome>
        <div class="settings-shell" data-testid="share-analytics-page">
            <header class="settings-header">
                <div class="settings-header-left">
                    <a href=move || format!("/dashboard/{}/share", workspace_id.get()) class="settings-back-link">
                        {move || i18n.t("share.backCenter")}
                    </a>
                    <h1 class="settings-title">{move || i18n.t("share.analyticsTitle")}</h1>
                </div>
            </header>
            <main class="settings-content">
                <p role="status" hidden=move || !loading.get()>{move || i18n.t("common.loading")}</p>
                <div hidden=move || error.get().is_none()><p role="alert">{move || error.get()}</p><button type="button" on:click=move |_| retry.update(|n| *n += 1)>{move || i18n.t("common.retry")}</button></div>
                <section class="settings-panel">
                    <div class="settings-usage-cards">
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">{move || i18n.t("analytics.totalViews")}</span>
                            <span class="settings-usage-value">
                                {move || if loading.get() || error.get().is_some() { i18n.t("common.unknown") } else { analytics.get().map(|a| a.total_views.to_string()).unwrap_or_else(|| i18n.t("common.unknown")) }}
                            </span>
                        </div>
                        <div class="settings-usage-card">
                            <span class="settings-usage-label">{move || i18n.t("analytics.visitors")}</span>
                            <span class="settings-usage-value">
                                {move || if loading.get() || error.get().is_some() { i18n.t("common.unknown") } else { analytics.get().map(|a| a.total_unique_visitors.to_string()).unwrap_or_else(|| i18n.t("common.unknown")) }}
                            </span>
                        </div>
                    </div>
                </section>
            </main>
        </div>
        </ProductChrome>
    }
}
