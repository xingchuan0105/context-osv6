use crate::api_base::poc_api_base;
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
        label: "Quick Chat (百炼 · qwen3.8-flash)",
        model_hint: "qwen3.8-flash",
        purpose: "quick_chat",
    },
    FixedProviderRow {
        id: "agent_llm",
        provider: "deepseek",
        label: "Agent 主模型 (DeepSeek · deepseek-v4-flash)",
        model_hint: "deepseek-v4-flash",
        purpose: "llm",
    },
    FixedProviderRow {
        id: "parse_llm",
        provider: "bailian",
        label: "文档解析模型 (百炼 · qwen3.7-flash)",
        model_hint: "qwen3.7-flash",
        purpose: "llm",
    },
    FixedProviderRow {
        id: "siliconflow",
        provider: "siliconflow",
        label: "向量与重排 (SiliconFlow · BAAI/bge-m3)",
        model_hint: "BAAI/bge-m3",
        purpose: "embedding",
    },
];

#[component]
pub fn ProvidersPanel() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let secrets = RwSignal::new(Vec::<ProviderSecretRow>::new());
    let loading = RwSignal::new(false);
    let refresh_gen = RwSignal::new(0_u64);

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
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            if let Ok(resp) = client.list_provider_secrets().await {
                secrets.set(resp.secrets);
            }
            loading.set(false);
        });
    });

    view! {
        <section class="settings-panel" aria-label="模型提供商与自备密钥" data-testid="providers-panel">
            <header class="settings-panel-header">
                <h2 class="settings-panel-title">"模型提供商密钥 (BYOK)"</h2>
                <p class="settings-panel-desc">
                    "配置您自己的 API Key。自备密钥将直接由本地或私有通道调用，系统绝不明文记录密钥。"
                </p>
            </header>
            <div class="settings-provider-list">
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[0] secrets=secrets token=token refresh_gen=refresh_gen/>
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[1] secrets=secrets token=token refresh_gen=refresh_gen/>
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[2] secrets=secrets token=token refresh_gen=refresh_gen/>
                <ProviderRowItem row=&FIXED_PROVIDER_ROWS[3] secrets=secrets token=token refresh_gen=refresh_gen/>
            </div>
        </section>
    }
}

#[component]
fn ProviderRowItem(
    row: &'static FixedProviderRow,
    secrets: RwSignal<Vec<ProviderSecretRow>>,
    token: RwSignal<String>,
    refresh_gen: RwSignal<u64>,
) -> impl IntoView {
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let action_error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

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
            action_error.set(Some("请输入有效的 API Key".to_string()));
            return;
        }
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth()
                .map(|a| a.token)
                .unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() || busy.get() {
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
                    secrets.update(|list| {
                        list.retain(|s| s.purpose != row.purpose);
                        list.push(ProviderSecretRow {
                            id: format!("local-sec-{}", row.id),
                            purpose: row.purpose.to_string(),
                            provider: row.provider.to_string(),
                            model_hint: Some(row.model_hint.to_string()),
                            is_active: Some(true),
                            revoked_at: None,
                        });
                    });
                    refresh_gen.update(|n| *n += 1);
                }
                Err(err) => {
                    action_error.set(Some(format!("保存失败：{err}")));
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
        if tok.is_empty() || busy.get() {
            return;
        }

        busy.set(true);
        action_error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.revoke_provider_secret(&secret_id).await {
                Ok(_) => {
                    secrets.update(|list| {
                        list.retain(|s| s.id != secret_id && s.purpose != row.purpose);
                    });
                    refresh_gen.update(|n| *n += 1);
                }
                Err(err) => {
                    action_error.set(Some(format!("撤销失败：{err}")));
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="settings-provider-row" data-testid=format!("provider-row-{}", row.id)>
            <div class="settings-provider-meta">
                <span class="settings-provider-name">{row.label}</span>
                <span class="settings-provider-model">{row.model_hint}</span>
            </div>
            <div class="settings-provider-action">
                {move || {
                    if let Some(sec) = active_secret.get() {
                        let sec_id = sec.id.clone();
                        view! {
                            <div class="settings-secret-status">
                                <span class="settings-status-badge" data-testid=format!("status-{}", row.id)>
                                    "已配置 (自备密钥)"
                                </span>
                                <button
                                    type="button"
                                    class="settings-btn-revoke"
                                    data-testid=format!("revoke-{}", row.id)
                                    disabled=move || busy.get()
                                    on:click=move |_| on_revoke(sec_id.clone())
                                >
                                    "移除密钥"
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
                                    placeholder="输入 API Key"
                                    node_ref=input_ref
                                />
                                <button
                                    type="button"
                                    class="settings-btn-save"
                                    data-testid=format!("save-{}", row.id)
                                    disabled=move || busy.get()
                                    on:click=on_save
                                >
                                    "保存"
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
