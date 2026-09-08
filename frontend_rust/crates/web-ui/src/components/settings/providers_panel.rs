use crate::api_base::poc_api_base;
use crate::i18n::{tf_now, t_now, use_i18n};
use leptos::prelude::*;
use web_sdk::{BrowserRestClient, ProviderSecretRow};

struct FixedProviderRow {
    id: &'static str,
    provider: &'static str,
    label: &'static str,
    model_hint: &'static str,
    purpose: &'static str,
}

const FIXED_PROVIDER_ROWS: &[FixedProviderRow] = &[
    FixedProviderRow {
        id: "quick_chat",
        provider: "bailian",
        label: "providers.quickChatLabel",
        model_hint: "qwen3.8-flash",
        purpose: "quick_chat",
    },
    FixedProviderRow {
        id: "agent_llm",
        provider: "deepseek",
        label: "providers.agentLabel",
        model_hint: "deepseek-v4-flash",
        purpose: "llm",
    },
    FixedProviderRow {
        id: "parse_llm",
        provider: "bailian",
        label: "providers.parseLabel",
        model_hint: "qwen3.7-flash",
        purpose: "llm",
    },
    FixedProviderRow {
        id: "siliconflow",
        provider: "siliconflow",
        label: "providers.embeddingLabel",
        model_hint: "BAAI/bge-m3",
        purpose: "embedding",
    },
];

#[component]
pub fn ProvidersPanel() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let i18n = use_i18n();
    let secrets = RwSignal::new(Vec::<ProviderSecretRow>::new());
    let loading = RwSignal::new(false);
    let refresh_gen = RwSignal::new(0_u64);
    let load_error = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let mut tok = token.get();
        if tok.is_empty() {
            if let Some(auth) = web_sdk::read_browser_auth() {
                tok = auth.token.clone();
                token.set(auth.token);
            } else {
                secrets.set(Vec::new());
                return;
            }
        }
        let _ = refresh_gen.get();
        loading.set(true);
        load_error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.list_provider_secrets().await {
                Ok(resp) => secrets.set(resp.secrets),
                Err(err) => load_error.set(Some(tf_now("providers.loadFailed", &[("error", &err.to_string())]))),
            }
            loading.set(false);
        });
    });

    view! {
        <section class="settings-panel" aria-label=move || i18n.t("providers.panelLabel") data-testid="providers-panel">
            <header class="settings-panel-header">
                <h2 class="settings-panel-title">{move || i18n.t("providers.title")}</h2>
                <p class="settings-panel-desc">
                    {move || i18n.t("providers.subtitle")}
                </p>
            </header>
            {move || load_error.get().map(|message| view! {
                <p role="alert" class="settings-error">{message}</p>
                <button type="button" disabled=move || loading.get() on:click=move |_| refresh_gen.update(|n| *n += 1)>{move || i18n.t("payment.refresh")}</button>
            })}
            <div class="settings-provider-list">
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[0] secrets=secrets token=token busy=loading load_error=load_error/>
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[1] secrets=secrets token=token busy=loading load_error=load_error/>
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[2] secrets=secrets token=token busy=loading load_error=load_error/>
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[3] secrets=secrets token=token busy=loading load_error=load_error/>
            </div>
        </section>
    }
}

#[component]
fn ProviderRowItem(
    row: &'static FixedProviderRow,
    secrets: RwSignal<Vec<ProviderSecretRow>>,
    token: RwSignal<String>,
    busy: RwSignal<bool>,
    load_error: RwSignal<Option<String>>,
) -> impl IntoView {
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let action_error = RwSignal::new(None::<String>);

    let active_secret = Signal::derive(move || {
        secrets.with(|list| {
            list.iter()
                .find(|s| {
                    s.purpose == row.purpose
                        && s.provider.eq_ignore_ascii_case(row.provider)
                        && s.revoked_at.is_none()
                        && s.is_active != Some(false)
                })
                .cloned()
        })
    });

    let on_save = move |_| {
        let val = input_ref
            .get()
            .map(|el| el.value())
            .unwrap_or_default()
            .trim()
            .to_string();
        if val.is_empty() {
            action_error.set(Some(t_now("providers.needKey")));
            return;
        }
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth()
                .map(|a| a.token)
                .unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() || busy.get() || load_error.get().is_some() {
            return;
        }

        busy.set(true);
        action_error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client
                .upsert_provider_secret(row.provider, &val, row.purpose, Some(row.model_hint))
                .await
            {
                Ok(_) => {
                    if let Some(el) = input_ref.get() {
                        el.set_value("");
                    }
                    match client.list_provider_secrets().await {
                        Ok(resp) => secrets.set(resp.secrets),
                        Err(err) => load_error.set(Some(tf_now("providers.loadFailed", &[("error", &err.to_string())]))),
                    }
                }
                Err(err) => {
                    action_error.set(Some(tf_now("providers.saveFailed", &[("error", &err.to_string())])));
                }
            }
            busy.set(false);
        });
    };

    let on_revoke = move |secret_id: String| {
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth()
                .map(|a| a.token)
                .unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() || busy.get() || load_error.get().is_some() {
            return;
        }

        busy.set(true);
        action_error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.revoke_provider_secret(&secret_id).await {
                Ok(_) => {
                    secrets.update(|list| {
                        list.retain(|s| s.id != secret_id);
                    });
                }
                Err(err) => {
                    action_error.set(Some(tf_now("providers.revokeFailed", &[("error", &err.to_string())])));
                }
            }
            busy.set(false);
        });
    };

    let i18n = use_i18n();
    view! {
        <div class="settings-provider-row" data-testid=format!("provider-row-{}", row.id)>
            <div class="settings-provider-meta">
                <span class="settings-provider-name">{move || i18n.t(row.label)}</span>
                <span class="settings-provider-model">{row.model_hint}</span>
            </div>
            <div class="settings-provider-action">
                {move || {
                    if let Some(sec) = active_secret.get() {
                        let sec_id = sec.id.clone();
                        view! {
                            <div class="settings-secret-status">
                                <span class="settings-status-badge" data-testid=format!("status-{}", row.id)>
                                    {move || i18n.t("providers.configured")}
                                </span>
                                <button
                                    type="button"
                                    class="settings-btn-revoke"
                                    data-testid=format!("revoke-{}", row.id)
                                    disabled=move || busy.get() || load_error.get().is_some()
                                    on:click=move |_| on_revoke(sec_id.clone())
                                >
                                    {move || i18n.t("providers.removeKey")}
                                </button>
                            </div>
                        }
                        .into_any()
                    } else {
                        view! {
                            <div class="settings-secret-form">
                                <input
                                    type="password"
                                    class="settings-key-input"
                                    data-testid=format!("input-{}", row.id)
                                    placeholder=move || i18n.t("providers.keyPlaceholder")
                                    node_ref=input_ref
                                />
                                <button
                                    type="button"
                                    class="settings-btn-save"
                                    data-testid=format!("save-{}", row.id)
                                    disabled=move || busy.get() || load_error.get().is_some()
                                    on:click=on_save
                                >
                                    {move || i18n.t("commonSave")}
                                </button>
                            </div>
                        }
                        .into_any()
                    }
                }}
            </div>
            {move || {
                action_error.get().map(|msg| {
                    view! {
                        <p class="settings-error" role="alert" data-testid=format!("error-{}", row.id)>
                            {msg}
                        </p>
                    }
                })
            }}
        </div>
    }
}
