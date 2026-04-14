//! UsageLimitCard — displays per-user LLM usage windows and breakdown
//!
//! PRD refs: §8.2 (required UI content), §13.2 (UI structure), §13.3 (visual states)

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use web_sdk::usage_limit::{UsageLimitResponse, UsageWindow};

use crate::api::api_base_url;
use crate::components::common::{ErrorBanner, LoadingMessage};
use crate::i18n::{Locale, choose};
use crate::load::run_once_after_hydration;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// PRD §13.3 visual thresholds: <70% normal, 70-90% warning, ≥90% danger, blocked
fn usage_bar_color(percent: f64, blocked: bool) -> &'static str {
    if blocked {
        "bg-red-600"
    } else if percent >= 90.0 {
        "bg-red-400"
    } else if percent >= 70.0 {
        "bg-yellow-500"
    } else {
        "bg-green-500"
    }
}

fn bar_width(percent: f64) -> String {
    format!("width: {}%", percent.min(100.0))
}

fn feature_label(locale: Locale, feature: &str) -> String {
    match feature {
        "chat" => choose(locale, "聊天", "Chat").to_string(),
        "answer" => choose(locale, "知识库回答", "RAG Answer").to_string(),
        "search" => choose(locale, "搜索", "Search").to_string(),
        "summary" => choose(locale, "摘要", "Summary").to_string(),
        "planner" => choose(locale, "规划", "Planner").to_string(),
        other => other.to_string(),
    }
}

fn scope_text(locale: Locale, scope: &web_sdk::usage_limit::UsageScope) -> String {
    match scope {
        web_sdk::usage_limit::UsageScope::PlanDefault { plan_id } => {
            let plan = if plan_id == "free" {
                choose(locale, "免费版", "Free")
            } else if plan_id == "pro" {
                choose(locale, "专业版", "Pro")
            } else {
                plan_id.as_str().into()
            };
            format!("{}{}", choose(locale, "配额来源：", "Quota from: "), plan)
        }
        web_sdk::usage_limit::UsageScope::UserOverride => choose(
            locale,
            "配额来源：自定义覆盖",
            "Quota from: custom override",
        )
        .to_string(),
    }
}

fn format_iso_datetime_compact(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let Some((date, time_part)) = trimmed.split_once('T') else {
        return trimmed.to_string();
    };

    let zone_index = time_part
        .find('Z')
        .or_else(|| time_part.find('+'))
        .or_else(|| time_part.find('-'))
        .unwrap_or(time_part.len());
    let without_zone = &time_part[..zone_index];
    let without_fraction = without_zone.split('.').next().unwrap_or(without_zone);
    let time_value = if without_fraction.len() >= 5 {
        &without_fraction[..5]
    } else {
        without_fraction
    };

    if time_value.is_empty() {
        date.to_string()
    } else {
        format!("{} {}", date, time_value)
    }
}

// ---------------------------------------------------------------------------
// Window progress sub-component
// ---------------------------------------------------------------------------

#[component]
fn WindowProgress(locale: Locale, label: String, window: UsageWindow) -> impl IntoView {
    let pct = window.percent_used;
    let unlimited = window.limit_units == 0;
    let blocked = window.blocked;
    let remaining = window.remaining_units;
    let blocked_until = window
        .blocked_until
        .as_deref()
        .map(format_iso_datetime_compact);
    let next_relief = window
        .next_relief_at
        .as_deref()
        .map(format_iso_datetime_compact);

    view! {
        <div class="space-y-1.5">
            // Row 1: label + numeric summary
            <div class="flex items-center justify-between text-sm">
                <span class="font-medium text-card-foreground">{label}</span>
                {if unlimited {
                    view! {
                        <span class="text-muted-foreground">
                            {choose(locale, "不限", "Unlimited").to_string()}
                        </span>
                    }.into_any()
                } else {
                    view! {
                        <span class="text-muted-foreground">
                            {format!(
                                "{} {} / {} · {} {} · {}{:.0}%",
                                choose(locale, "已用", "Used"),
                                window.used_units,
                                window.limit_units,
                                choose(locale, "剩余", "Remaining"),
                                remaining,
                                choose(locale, "", ""),
                                pct,
                            )}
                        </span>
                    }.into_any()
                }}
            </div>

            // Row 2: progress bar
            {if !unlimited {
                view! {
                    <div class="h-2.5 w-full overflow-hidden rounded-full bg-muted">
                        <div
                            class=usage_bar_color(pct, blocked)
                            style=bar_width(pct)
                        ></div>
                    </div>
                }.into_any()
            } else {
                view! { <div /> }.into_any()
            }}

            // Row 3: status line — blocked_until recovery or next relief
            {if blocked {
                view! {
                    <p class="text-xs font-medium text-red-600">
                        {if let Some(until) = &blocked_until {
                            format!(
                                "{}{}{} ({}/{})",
                                choose(locale, "已达上限。预计恢复时间：", "Limit reached. Expected resume: "),
                                until,
                                choose(locale, "，当前用量 ", ""),
                                window.used_units,
                                window.limit_units,
                            )
                        } else {
                            format!(
                                "{} ({}/{})",
                                choose(locale, "已达上限，请等待额度恢复", "Limit reached — wait for quota relief"),
                                window.used_units,
                                window.limit_units,
                            )
                        }}
                    </p>
                }.into_any()
            } else if let Some(relief) = &next_relief {
                view! {
                    <p class="text-xs text-muted-foreground">
                        {format!(
                            "{}{}",
                            choose(locale, "最早释放：", "Next relief: "),
                            relief,
                        )}
                    </p>
                }.into_any()
            } else {
                view! { <div /> }.into_any()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Breakdown sub-component
// ---------------------------------------------------------------------------

#[component]
fn BreakdownGrid(locale: Locale, items: Vec<(String, i64)>) -> impl IntoView {
    view! {
        <div>
            <p class="mb-2 text-sm font-medium text-card-foreground">
                {choose(locale, "按功能分类", "Breakdown by feature")}
            </p>
            <div class="grid gap-2 md:grid-cols-2">
                {items.into_iter().map(|(feature, units)| {
                    view! {
                        <div class="flex items-center justify-between rounded-lg border border-border bg-muted/30 px-3 py-2 text-sm">
                            <span class="text-muted-foreground">
                                {feature_label(locale, &feature)}
                            </span>
                            <span class="font-medium text-card-foreground">
                                {format!("{} {}", units, choose(locale, "单位", "units"))}
                            </span>
                        </div>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Scope explanation (PRD §8.2: included/excluded features)
// ---------------------------------------------------------------------------

#[component]
fn ScopeExplanation(locale: Locale, scope: web_sdk::usage_limit::UsageScope) -> impl IntoView {
    let scope_origin = scope_text(locale, &scope);

    view! {
        <div class="space-y-2">
            <p class="text-xs text-muted-foreground">{scope_origin}</p>
            <div class="text-xs text-muted-foreground space-y-1">
                <p>
                    {format!(
                        "{} summary · planner · answer · search · chat",
                        choose(locale, "计入用量：", "Metered: "),
                    )}
                </p>
                <p>
                    {format!(
                        "{} MinerU · Embedding · Rerank",
                        choose(locale, "不计入：", "Excluded: "),
                    )}
                </p>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Main UsageLimitCard
// ---------------------------------------------------------------------------

#[component]
pub fn UsageLimitCard() -> impl IntoView {
    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let (usage_data, set_usage_data) = signal(Option::<UsageLimitResponse>::None);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(String::new());
    let (loaded_token, set_loaded_token) = signal(String::new());

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || auth_for_load.token.get().unwrap_or_default(),
        loaded_token,
        set_loaded_token,
        move || {
            let Some(token) = auth.token.get_untracked() else {
                return;
            };
            set_loading.set(true);
            set_error.set(String::new());
            let client = web_sdk::ApiClient::new(api_base_url()).with_auth(token);
            spawn(async move {
                match client.get_usage_limit().await {
                    Ok(data) => set_usage_data.set(Some(data)),
                    Err(e) => set_error.set(format!(
                        "{}: {}",
                        choose(
                            locale.get_untracked(),
                            "加载用量信息失败",
                            "Failed to load usage"
                        ),
                        e
                    )),
                }
                set_loading.set(false);
            });
        },
    );

    let has_data = move || usage_data.get().is_some() && !loading.get() && error.get().is_empty();

    view! {
        <div class="app-surface-card">
            <h3 class="mb-4 text-lg font-semibold text-card-foreground">
                {move || choose(locale.get(), "个人用量", "Personal Usage")}
            </h3>

            <Show when=move || loading.get()>
                <LoadingMessage
                    message={
                        choose(
                            locale.get_untracked(),
                            "正在加载用量...",
                            "Loading usage...",
                        )
                        .to_string()
                    }
                />
            </Show>

            <Show when=move || !error.get().is_empty()>
                <ErrorBanner message={error.get()} />
            </Show>

            <Show when=has_data>
                {move || {
                    let Some(resp) = usage_data.get() else {
                        return view! { <div /> }.into_any();
                    };
                    let loc = locale.get();
                    let enabled = resp.policy.enabled;
                    let has_estimated = resp.has_estimated_usage;
                    let breakdown: Vec<(String, i64)> = resp.breakdown.clone().into_iter().collect();
                    let has_breakdown = !breakdown.is_empty();
                    let scope = resp.scope.clone();

                    view! {
                        <div class="space-y-4">
                            // Scope explanation (PRD §8.2)
                            <ScopeExplanation locale=loc scope={scope} />

                            {if !enabled {
                                view! {
                                    <p class="text-sm text-muted-foreground">
                                        {choose(loc, "用量限制未启用。", "Usage limits are not enabled.")}
                                    </p>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="space-y-4">
                                        // 5h window
                                        <WindowProgress
                                            locale=loc
                                            label={choose(loc, "5 小时限额", "5-hour limit").to_string()}
                                            window={resp.windows.rolling_5h.clone()}
                                        />
                                        // 7d window
                                        <WindowProgress
                                            locale=loc
                                            label={choose(loc, "7 天限额", "7-day limit").to_string()}
                                            window={resp.windows.rolling_7d.clone()}
                                        />
                                    </div>
                                }.into_any()
                            }}

                            // Breakdown
                            {if has_breakdown {
                                view! {
                                    <BreakdownGrid locale=loc items={breakdown} />
                                }.into_any()
                            } else {
                                view! { <div /> }.into_any()
                            }}

                            // Estimated usage notice
                            {if has_estimated {
                                view! {
                                    <p class="text-xs text-muted-foreground italic">
                                        {choose(loc, "* 部分用量为估算值", "* Some usage is estimated")}
                                    </p>
                                }.into_any()
                            } else {
                                view! { <div /> }.into_any()
                            }}
                        </div>
                    }.into_any()
                }}
            </Show>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_bar_color_normal_below_70() {
        assert_eq!(usage_bar_color(50.0, false), "bg-green-500");
        assert_eq!(usage_bar_color(0.0, false), "bg-green-500");
        assert_eq!(usage_bar_color(69.9, false), "bg-green-500");
    }

    #[test]
    fn usage_bar_color_warning_70_to_90() {
        assert_eq!(usage_bar_color(70.0, false), "bg-yellow-500");
        assert_eq!(usage_bar_color(80.0, false), "bg-yellow-500");
        assert_eq!(usage_bar_color(89.9, false), "bg-yellow-500");
    }

    #[test]
    fn usage_bar_color_danger_above_90() {
        assert_eq!(usage_bar_color(90.0, false), "bg-red-400");
        assert_eq!(usage_bar_color(100.0, false), "bg-red-400");
    }

    #[test]
    fn usage_bar_color_blocked_overrides_all() {
        assert_eq!(usage_bar_color(10.0, true), "bg-red-600");
        assert_eq!(usage_bar_color(95.0, true), "bg-red-600");
    }

    #[test]
    fn bar_width_capped_at_100() {
        assert_eq!(bar_width(50.0), "width: 50%");
        assert_eq!(bar_width(100.0), "width: 100%");
        assert_eq!(bar_width(150.0), "width: 100%");
    }

    #[test]
    fn feature_label_known_features() {
        assert_eq!(feature_label(Locale::En, "chat"), "Chat");
        assert_eq!(feature_label(Locale::ZhCn, "chat"), "聊天");
        assert_eq!(feature_label(Locale::En, "answer"), "RAG Answer");
        assert_eq!(feature_label(Locale::En, "search"), "Search");
        assert_eq!(feature_label(Locale::En, "summary"), "Summary");
        assert_eq!(feature_label(Locale::En, "planner"), "Planner");
    }

    #[test]
    fn feature_label_unknown_passthrough() {
        assert_eq!(
            feature_label(Locale::En, "custom_feature"),
            "custom_feature"
        );
    }
}
