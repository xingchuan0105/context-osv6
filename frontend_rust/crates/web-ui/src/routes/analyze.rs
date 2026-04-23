//! Workspace analyze page - share analytics only.

use leptos::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use leptos::task::spawn;
#[cfg(target_arch = "wasm32")]
use leptos::task::spawn_local as spawn;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_params_map};
use web_sdk::ApiClient;
use web_sdk::dtos::{AccessLogsResponse, ShareAnalyticsResponse, ShareSettings};

use crate::api::api_base_url;
use crate::components::share::{ShareAccessLogs, ShareAnalytics};
use crate::components::{NoticeBanner, NoticeTone};
use crate::i18n::choose;
use crate::load::run_once_after_hydration;
use crate::state::auth::use_auth_state;
use crate::state::ui_prefs::use_ui_prefs_state;

stylance::import_crate_style!(
    #[allow(dead_code)]
    shared_page_style,
    "src/routes/shared/shared_pages.module.css"
);

fn workspace_share_enabled(settings: &ShareSettings) -> bool {
    !settings.share_token.trim().is_empty()
        && !settings.access_level.eq_ignore_ascii_case("private")
}

#[component]
pub fn WorkspaceAnalyzePage() -> impl IntoView {
    let params = use_params_map();
    let workspace_id = move || params.get().get("notebook_id").unwrap_or_default();

    let auth = use_auth_state();
    let locale = use_ui_prefs_state().locale;
    let location = use_location();
    let is_preview_route = Memo::new(move |_| location.pathname.get().starts_with("/preview/live"));

    let workspace_href = Memo::new(move |_| {
        let wid = workspace_id();
        if wid.is_empty() {
            if is_preview_route.get() {
                "/preview/live/dashboard".to_string()
            } else {
                "/dashboard".to_string()
            }
        } else if is_preview_route.get() {
            format!("/preview/live/workspace/{wid}")
        } else {
            format!("/dashboard/{wid}")
        }
    });
    let share_href = Memo::new(move |_| {
        let wid = workspace_id();
        if wid.is_empty() {
            if is_preview_route.get() {
                "/preview/live/dashboard".to_string()
            } else {
                "/dashboard".to_string()
            }
        } else if is_preview_route.get() {
            format!("/preview/live/workspace/{wid}/share")
        } else {
            format!("/dashboard/{wid}/share")
        }
    });

    let (settings, set_settings) = signal(Option::<ShareSettings>::None);
    let (analytics, set_analytics) = signal(Option::<ShareAnalyticsResponse>::None);
    let (logs, set_logs) = signal(Option::<AccessLogsResponse>::None);
    let (error, set_error) = signal(String::new());
    let (loaded_key, set_loaded_key) = signal(String::new());

    let auth_for_load = auth.clone();
    run_once_after_hydration(
        move || {
            auth_for_load
                .token
                .get()
                .map(|value| format!("{}:{}", value, workspace_id()))
                .unwrap_or_default()
        },
        loaded_key,
        set_loaded_key,
        move || {
            let wid = workspace_id();
            if wid.is_empty() {
                return;
            }
            let Some(token) = auth.token.get_untracked() else {
                return;
            };

            let client = ApiClient::new(api_base_url()).with_auth(token);
            let locale_now = locale.get_untracked();

            let client_for_settings = client.clone();
            let wid_for_settings = wid.clone();
            spawn(async move {
                match client_for_settings
                    .get_share_settings(&wid_for_settings)
                    .await
                {
                    Ok(resp) => set_settings.set(Some(resp)),
                    Err(fetch_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale_now,
                                "加载分享设置失败",
                                "Failed to load share settings"
                            ),
                            fetch_error
                        ));
                    }
                }
            });

            let client_for_analytics = client.clone();
            let wid_for_analytics = wid.clone();
            spawn(async move {
                match client_for_analytics
                    .get_share_analytics(&wid_for_analytics)
                    .await
                {
                    Ok(resp) => set_analytics.set(Some(resp)),
                    Err(fetch_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(
                                locale_now,
                                "加载分享分析失败",
                                "Failed to load share analytics"
                            ),
                            fetch_error
                        ));
                    }
                }
            });

            let client_for_logs = client;
            spawn(async move {
                match client_for_logs.get_access_logs(&wid).await {
                    Ok(resp) => set_logs.set(Some(resp)),
                    Err(fetch_error) => {
                        set_error.set(format!(
                            "{}: {}",
                            choose(locale_now, "加载访问日志失败", "Failed to load access logs"),
                            fetch_error
                        ));
                    }
                }
            });
        },
    );

    let sharing_enabled = Memo::new(move |_| {
        settings
            .get()
            .as_ref()
            .map(workspace_share_enabled)
            .unwrap_or(false)
    });

    view! {
        <div class=shared_page_style::page_shell>
            <div class=shared_page_style::page_inner>
                <div class=shared_page_style::page_stack>
                    <div class=shared_page_style::page_heading>
                        <A href=move || workspace_href.get() attr:class=shared_page_style::back_link>
                            <svg class=shared_page_style::back_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M15 18l-6-6 6-6"/>
                            </svg>
                            <span>{move || choose(locale.get(), "返回 Workspace", "Back to Workspace")}</span>
                        </A>
                        <div class=shared_page_style::page_heading_row>
                            <div>
                                <h1 class=shared_page_style::page_title>
                                    {move || choose(locale.get(), "分享分析", "Share Analytics")}
                                </h1>
                                <p class=shared_page_style::page_subtitle>
                                    {move || choose(
                                        locale.get(),
                                        "查看当前 Workspace 的分享状态、访问趋势和最近访问记录。",
                                        "Review share status, traffic trends, and recent access activity for this workspace."
                                    )}
                                </p>
                            </div>
                            <A href=move || share_href.get() attr:class=shared_page_style::primary_button>
                                {move || choose(locale.get(), "前往 Share", "Go to Share")}
                            </A>
                        </div>
                    </div>

                    <Show when=move || !error.get().is_empty()>
                        <NoticeBanner message=error.get() tone=NoticeTone::Warning />
                    </Show>

                    <Show
                        when=move || settings.get().is_some()
                        fallback=move || view! {
                            <div class=shared_page_style::loading_state>
                                {move || choose(locale.get(), "正在加载分析...", "Loading analytics...")}
                            </div>
                        }
                    >
                        <Show
                            when=move || sharing_enabled.get()
                            fallback=move || view! {
                                <section class=format!("{} {}", shared_page_style::card, shared_page_style::card_pad)>
                                    <div class=shared_page_style::section_intro>
                                        <h2 class=shared_page_style::section_title>
                                            {move || choose(locale.get(), "还没有可分析的分享数据", "No share analytics yet")}
                                        </h2>
                                        <p class=shared_page_style::section_desc>
                                            {move || choose(
                                                locale.get(),
                                                "先在 Share 页面启用分享，再回到这里查看访问量、独立访客和访问日志。",
                                                "Enable sharing first, then return here to review views, unique visitors, and access logs."
                                            )}
                                        </p>
                                    </div>
                                    <div class=shared_page_style::action_row>
                                        <A href=move || share_href.get() attr:class=shared_page_style::primary_button>
                                            {move || choose(locale.get(), "前往 Share", "Go to Share")}
                                        </A>
                                    </div>
                                </section>
                            }
                        >
                            {move || {
                                let current_settings = settings.get().unwrap();
                                view! {
                                    <section class=format!("{} {}", shared_page_style::card, shared_page_style::card_pad)>
                                        <div class=shared_page_style::section_intro>
                                            <h2 class=shared_page_style::section_title>
                                                {move || choose(locale.get(), "分享状态", "Share Status")}
                                            </h2>
                                            <p class=shared_page_style::section_desc>
                                                {move || choose(
                                                    locale.get(),
                                                    "当前只展示分享相关分析，不包含 token 或成本统计。",
                                                    "This page is intentionally limited to share analytics and does not include token or cost metrics."
                                                )}
                                            </p>
                                        </div>
                                        <div class=shared_page_style::stats_grid>
                                            <div class=shared_page_style::stats_card>
                                                <div class=shared_page_style::stats_label>{move || choose(locale.get(), "访问级别", "Access Level")}</div>
                                                <div class=shared_page_style::stats_value>{current_settings.access_level.clone()}</div>
                                            </div>
                                            <div class=shared_page_style::stats_card>
                                                <div class=shared_page_style::stats_label>{move || choose(locale.get(), "允许下载", "Allow Download")}</div>
                                                <div class=shared_page_style::stats_value>
                                                    {if current_settings.allow_download {
                                                        choose(locale.get_untracked(), "已开启", "Enabled").to_string()
                                                    } else {
                                                        choose(locale.get_untracked(), "未开启", "Disabled").to_string()
                                                    }}
                                                </div>
                                            </div>
                                            <div class=shared_page_style::stats_card>
                                                <div class=shared_page_style::stats_label>{move || choose(locale.get(), "过期时间", "Expires At")}</div>
                                                <div class=shared_page_style::stats_value>
                                                    {current_settings.expires_at.clone().unwrap_or_else(|| choose(locale.get_untracked(), "未设置", "Not set").to_string())}
                                                </div>
                                            </div>
                                        </div>
                                    </section>

                                    <Show
                                        when=move || analytics.get().is_some()
                                        fallback=move || view! {
                                            <div class=shared_page_style::loading_state>
                                                {move || choose(locale.get(), "正在加载指标...", "Loading metrics...")}
                                            </div>
                                        }
                                    >
                                        {move || analytics.get().map(|payload| view! { <ShareAnalytics analytics=payload /> })}
                                    </Show>

                                    <Show
                                        when=move || logs.get().is_some()
                                        fallback=move || view! {
                                            <div class=shared_page_style::loading_state>
                                                {move || choose(locale.get(), "正在加载访问日志...", "Loading access logs...")}
                                            </div>
                                        }
                                    >
                                        {move || logs.get().map(|payload| view! { <ShareAccessLogs logs=payload.logs /> })}
                                    </Show>
                                }
                                    .into_any()
                            }}
                        </Show>
                    </Show>
                </div>
            </div>
        </div>
    }
}
