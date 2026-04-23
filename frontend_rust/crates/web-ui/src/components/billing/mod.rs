//! Billing components - Subscription, usage, and plan management

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use web_sdk::ApiClient;
use web_sdk::dtos::{PlanRow, PlansResponse, SubscriptionResponse, UsageResponse};

use crate::api::api_base_url;
use crate::i18n::{Locale, choose};
use crate::load::run_once_after_hydration;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

/// Format price from cents to dollars string
fn format_price(cents: i64) -> String {
    let dollars = cents as f64 / 100.0;
    format!("${:.2}", dollars)
}

/// Calculate percentage and return color class based on usage
fn usage_color_class(percentage: f64) -> &'static str {
    if percentage >= 90.0 {
        "bg-red-500"
    } else if percentage >= 70.0 {
        "bg-yellow-500"
    } else {
        "bg-green-500"
    }
}

/// Helper for number formatting
fn format_number(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn subscription_status_label(locale: Locale, status: &str) -> String {
    match status {
        "active" => choose(locale, "有效", "active").to_string(),
        "past_due" => choose(locale, "逾期", "past_due").to_string(),
        "canceled" => choose(locale, "已取消", "canceled").to_string(),
        _ => status.to_string(),
    }
}

fn feature_label(locale: Locale, feature: &str) -> String {
    let translate_metric = |metric: &str| match metric.trim() {
        "embedding_tokens" => choose(locale, "嵌入令牌", "embedding tokens").to_string(),
        "llm_input_tokens" => choose(locale, "模型输入令牌", "LLM input tokens").to_string(),
        "llm_output_tokens" => choose(locale, "模型输出令牌", "LLM output tokens").to_string(),
        "pages_processed" => choose(locale, "处理页数", "pages processed").to_string(),
        "storage_bytes" => choose(locale, "存储空间", "storage").to_string(),
        other => other.to_string(),
    };

    if let Some((metric, value)) = feature.split_once(':') {
        let value = value.trim();
        let translated_value = if value.eq_ignore_ascii_case("unlimited") {
            choose(locale, "不限", "unlimited").to_string()
        } else {
            value.to_string()
        };
        return format!("{}：{}", translate_metric(metric), translated_value);
    }

    feature.to_string()
}

// ----------------------------------------------------------------------------
// CurrentPlanSection component - shows current subscription info
// ----------------------------------------------------------------------------

#[component]
pub fn CurrentPlanSection(
    subscription: Option<SubscriptionResponse>,
    _current_plan_id: Option<String>,
    plans: Vec<PlanRow>,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    // Clone subscription for each closure to avoid move issues
    let sub_for_name = subscription.clone();
    let sub_for_status = subscription.clone();
    let sub_for_period = subscription.clone();
    let sub_for_has = subscription.clone();
    let sub_for_display = subscription.clone();

    let plan_name = move || {
        sub_for_name
            .as_ref()
            .and_then(|sub| {
                plans
                    .iter()
                    .find(|p| p.id == sub.plan_id)
                    .map(|p| p.name.clone())
            })
            .unwrap_or_else(|| choose(locale.get(), "暂无方案", "No plan").to_string())
    };

    let status_color = move || {
        sub_for_status
            .as_ref()
            .map(|s| match s.status.as_str() {
                "active" => "bg-emerald-100 text-emerald-800",
                "past_due" => "bg-red-100 text-red-800",
                "canceled" => "bg-muted text-muted-foreground",
                _ => "bg-amber-100 text-amber-800",
            })
            .unwrap_or("bg-muted text-muted-foreground")
    };

    let period_end = move || {
        sub_for_period
            .as_ref()
            .map(|s| {
                if s.current_period_end.len() >= 10 {
                    s.current_period_end[..10].to_string()
                } else {
                    s.current_period_end.clone()
                }
            })
            .unwrap_or_else(|| choose(locale.get(), "暂无", "N/A").to_string())
    };

    let has_subscription = move || sub_for_has.is_some();

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "当前方案", "Current Plan")}
            </h3>

            <Show
                when=has_subscription
                fallback=move || view! {
                    <div class="text-muted-foreground">{move || choose(locale.get(), "当前没有有效订阅", "No active subscription")}</div>
                }
            >
                <div class="space-y-3">
                    <div class="flex items-center justify-between">
                        <span class="text-muted-foreground">{move || choose(locale.get(), "方案", "Plan")}</span>
                        <span class="font-medium text-foreground">{plan_name()}</span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="text-muted-foreground">{move || choose(locale.get(), "状态", "Status")}</span>
                        <span class={format!("inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium {}", status_color())}>
                            {sub_for_display
                                .as_ref()
                                .map(|s| subscription_status_label(locale.get(), &s.status))
                                .unwrap_or_default()}
                        </span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="text-muted-foreground">{move || choose(locale.get(), "续费日期", "Renews on")}</span>
                        <span class="text-foreground">{period_end()}</span>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// UsageSection component - shows usage metrics with progress bars
// ----------------------------------------------------------------------------

#[component]
pub fn UsageSection(usage: Option<UsageResponse>) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    // Clone usage for each closure to avoid move issues
    let usage_tokens = usage.clone();
    let usage_docs = usage.clone();
    let usage_has = usage.clone();
    let usage_view = usage.clone();

    let tokens_pct = move || {
        usage_tokens
            .as_ref()
            .map(|u| {
                if u.limit_tokens > 0 {
                    (u.used_tokens as f64 / u.limit_tokens as f64 * 100.0).min(100.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
    };

    let docs_pct = move || {
        usage_docs
            .as_ref()
            .map(|u| {
                if u.limit_documents > 0 {
                    (u.used_documents as f64 / u.limit_documents as f64 * 100.0).min(100.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
    };

    let has_usage = move || usage_has.is_some();

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "用量", "Usage")}
            </h3>

            <Show when=has_usage fallback=move || view! {
                <div class="text-muted-foreground">{move || choose(locale.get(), "正在加载用量...", "Loading usage...")}</div>
            }>
                <div class="space-y-6">
                    {/* Tokens usage */}
                    <div>
                        <div class="flex items-center justify-between mb-2">
                            <span class="text-sm font-medium text-foreground">
                                {move || choose(locale.get(), "令牌", "Tokens")}
                            </span>
                            <span class="text-sm text-muted-foreground">
                                {usage_view.as_ref().map(|u| format_number(u.used_tokens)).unwrap_or_default()}
                                {" / "}
                                {usage_view.as_ref().map(|u| format_number(u.limit_tokens)).unwrap_or_default()}
                            </span>
                        </div>
                        <div class="app-progress-track w-full">
                            <div
                                class={format!("h-2 rounded-full transition-all {}", usage_color_class(tokens_pct()))}
                                style={format!("width: {}%", tokens_pct() as i32)}
                            ></div>
                        </div>
                        <div class="mt-1 text-right text-xs text-muted-foreground">
                            {format!("{:.0}%", tokens_pct())}
                        </div>
                    </div>

                        {/* Documents usage */}
                    <div>
                        <div class="flex items-center justify-between mb-2">
                            <span class="text-sm font-medium text-foreground">
                                {move || choose(locale.get(), "文档", "Documents")}
                            </span>
                            <span class="text-sm text-muted-foreground">
                                {usage_view.as_ref().map(|u| u.used_documents.to_string()).unwrap_or_default()}
                                {" / "}
                                {usage_view.as_ref().map(|u| u.limit_documents.to_string()).unwrap_or_default()}
                            </span>
                        </div>
                        <div class="app-progress-track w-full">
                            <div
                                class={format!("h-2 rounded-full transition-all {}", usage_color_class(docs_pct()))}
                                style={format!("width: {}%", docs_pct() as i32)}
                            ></div>
                        </div>
                        <div class="mt-1 text-right text-xs text-muted-foreground">
                            {format!("{:.0}%", docs_pct())}
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// PlanCard component - individual plan display
// ----------------------------------------------------------------------------

#[component]
pub fn PlanCard(plan: PlanRow, is_current_plan: bool) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let has_current_plan = move || is_current_plan;

    view! {
        <div class="app-inline-surface flex h-full flex-col">
            <div class="flex-1">
                <h4 class="text-lg font-semibold text-card-foreground">{plan.name.clone()}</h4>
                <p class="mt-1 text-2xl font-bold text-card-foreground">
                    {format_price(plan.price)}
                    <span class="text-sm font-normal text-muted-foreground">
                        {move || choose(locale.get(), " / 月", " / month")}
                    </span>
                </p>
                <ul class="mt-4 space-y-2">
                    {plan.features.iter().map(|feature| {
                        view! {
                            <li class="flex items-start">
                                <svg class="w-5 h-5 text-green-500 shrink-0 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>
                                </svg>
                                <span class="text-sm text-muted-foreground">{feature_label(locale.get(), feature)}</span>
                            </li>
                        }
                    }).collect_view()}
                </ul>
            </div>
            <div class="mt-6">
                <Show when=has_current_plan fallback=move || view! {
                    <a href="/settings" class="block w-full rounded-xl bg-primary px-4 py-2 text-center text-sm font-medium text-primary-foreground hover:bg-primary/90">
                        {move || choose(locale.get(), "升级方案", "Upgrade Plan")}
                    </a>
                }>
                    <div class="w-full rounded-xl bg-muted px-4 py-2 text-center text-sm font-medium text-muted-foreground">
                        {move || choose(locale.get(), "当前方案", "Current Plan")}
                    </div>
                </Show>
            </div>
        </div>
    }
}

// ----------------------------------------------------------------------------
// PlansSection component - shows all available plans
// ----------------------------------------------------------------------------

#[component]
pub fn PlansSection(
    plans: Vec<PlanRow>,
    current_plan_id: Option<String>,
    loading: bool,
) -> impl IntoView {
    let locale = use_ui_prefs_state().locale;
    let is_loading = move || loading;
    let no_plans = {
        let plans_copy = plans.clone();
        move || !loading && plans_copy.is_empty()
    };
    let has_plans = {
        let plans_copy = plans.clone();
        move || !loading && !plans_copy.is_empty()
    };

    // Pre-render plan cards to avoid closure issues
    let plan_views = {
        let plans_copy = plans.clone();
        let current_plan_id_clone = current_plan_id.clone();
        move || {
            plans_copy
                .clone()
                .into_iter()
                .map(|plan| {
                    let is_current = current_plan_id_clone
                        .as_ref()
                        .map(|id| id == &plan.id)
                        .unwrap_or(false);
                    view! {
                        <PlanCard
                            plan={plan}
                            is_current_plan={is_current}
                        />
                    }
                })
                .collect_view()
        }
    };

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "可用方案", "Available Plans")}
            </h3>

            <Show when=is_loading fallback=move || view! { <div></div> }>
                <div class="app-empty-state">
                    <div class="text-muted-foreground">
                        {move || choose(locale.get(), "正在加载方案...", "Loading plans...")}
                    </div>
                </div>
            </Show>

            <Show when=no_plans fallback=move || view! { <div></div> }>
                <div class="app-empty-state">
                    <div class="text-muted-foreground">
                        {move || choose(locale.get(), "暂无可用方案", "No plans available")}
                    </div>
                </div>
            </Show>

            <Show when=has_plans fallback=move || view! { <div></div> }>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                    {plan_views()}
                </div>
            </Show>
        </div>
    }
}

// ----------------------------------------------------------------------------
// SettingsTab enum - exported for use in settings page
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum SettingsTab {
    Billing,
    Profile,
    Notifications,
}

// ----------------------------------------------------------------------------
// BillingPanel component - main billing settings page
// ----------------------------------------------------------------------------

#[component]
pub fn BillingPanel() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;

    // State signals
    let (subscription, set_subscription) = signal(Option::<SubscriptionResponse>::None);
    let (usage, set_usage) = signal(Option::<UsageResponse>::None);
    let (plans, set_plans) = signal(Vec::<PlanRow>::new());
    let (loading_plans, set_loading_plans) = signal(false);
    let (error, set_error) = signal(String::new());
    let (manage_loading, set_manage_loading) = signal(false);
    let (loaded_token, set_loaded_token) = signal(String::new());

    // Fetch data on mount
    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || auth_for_load.token.get().unwrap_or_default(),
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            let client = ApiClient::new(api_base_url()).with_auth(token);

            let client_sub = client.clone();
            spawn(async move {
                if let Ok(resp) = client_sub.get_subscription().await {
                    set_subscription.set(Some(resp));
                }
            });

            let client_usage = client.clone();
            spawn(async move {
                if let Ok(resp) = client_usage.get_usage().await {
                    set_usage.set(Some(resp));
                }
            });

            set_loading_plans.set(true);
            spawn(async move {
                match client.list_plans().await {
                    Ok(PlansResponse { plans: plans_list }) => {
                        set_plans.set(plans_list);
                    }
                    Err(_) => {
                        set_error.set(
                            choose(
                                locale.get_untracked(),
                                "加载方案失败",
                                "Failed to load plans",
                            )
                            .to_string(),
                        );
                    }
                }
                set_loading_plans.set(false);
            });
        },
    );

    // Get current plan ID
    let current_plan_id = move || subscription.get().as_ref().map(|s| s.plan_id.clone());

    // Handle manage subscription (portal session)
    let handle_manage_subscription = move |_| {
        let token = match auth.token.get() {
            Some(t) => t,
            None => return,
        };

        set_manage_loading.set(true);
        set_error.set(String::new());

        let client = ApiClient::new(api_base_url()).with_auth(token);

        spawn(async move {
            match client.create_portal_session().await {
                Ok(resp) => {
                    set_manage_loading.set(false);
                    if let Some(url) = resp.get("url").and_then(|v| v.as_str()) {
                        if let Some(window) = web_sys::window() {
                            if window.location().set_href(url).is_err() {
                                set_error.set(
                                    choose(
                                        locale.get_untracked(),
                                        "打开订阅管理入口失败",
                                        "Failed to open billing portal",
                                    )
                                    .to_string(),
                                );
                            }
                        }
                    }
                }
                Err(_) => {
                    set_manage_loading.set(false);
                    set_error.set(
                        choose(
                            locale.get_untracked(),
                            "创建订阅管理入口失败",
                            "Failed to create portal session",
                        )
                        .to_string(),
                    );
                }
            }
        });
    };

    view! {
        <div class="space-y-6">
            <div
                class="app-notice-banner border-red-200 bg-red-50 text-red-800"
                class=("hidden", move || error.get().is_empty())
            >
                {move || error.get()}
            </div>

            {/* Current Plan section with Manage button */}
            <div class="app-surface-card">
                <div class="flex items-center justify-between mb-4">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "当前方案", "Current Plan")}
                    </h3>
                    <button
                        class="app-button-primary"
                        disabled=move || manage_loading.get()
                        on:click=handle_manage_subscription
                    >
                        {move || if manage_loading.get() {
                            choose(locale.get(), "加载中...", "Loading...")
                        } else {
                            choose(locale.get(), "管理订阅", "Manage Subscription")
                        }}
                    </button>
                </div>

                <div class="space-y-3">
                    <div class="flex items-center justify-between">
                        <span class="text-muted-foreground">{move || choose(locale.get(), "方案", "Plan")}</span>
                        <span class="font-medium text-foreground">
                            {move || {
                                subscription
                                    .get()
                                    .and_then(|current| {
                                        plans
                                            .get()
                                            .iter()
                                            .find(|plan| plan.id == current.plan_id)
                                            .map(|plan| plan.name.clone())
                                    })
                                    .unwrap_or_else(|| choose(locale.get(), "未开通", "Not active").to_string())
                            }}
                        </span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="text-muted-foreground">{move || choose(locale.get(), "状态", "Status")}</span>
                        <span class="font-medium text-foreground">
                            {move || {
                                subscription
                                    .get()
                                    .map(|current| subscription_status_label(locale.get(), &current.status))
                                    .unwrap_or_else(|| choose(locale.get(), "未开通", "Not active").to_string())
                            }}
                        </span>
                    </div>
                    <div class="flex items-center justify-between">
                        <span class="text-muted-foreground">{move || choose(locale.get(), "续费日期", "Renews on")}</span>
                        <span class="text-foreground">
                            {move || {
                                subscription
                                    .get()
                                    .map(|current| {
                                        if current.current_period_end.len() >= 10 {
                                            current.current_period_end[..10].to_string()
                                        } else {
                                            current.current_period_end
                                        }
                                    })
                                    .unwrap_or_else(|| choose(locale.get(), "暂无", "N/A").to_string())
                            }}
                        </span>
                    </div>
                </div>
            </div>

            {/* Usage section */}
            {move || {
                view! { <UsageSection usage={usage.get()} /> }
            }}

            {/* Plans section */}
            {move || {
                view! {
                    <PlansSection
                        plans={plans.get()}
                        current_plan_id={current_plan_id()}
                        loading={loading_plans.get()}
                    />
                }
            }}
        </div>
    }
}
