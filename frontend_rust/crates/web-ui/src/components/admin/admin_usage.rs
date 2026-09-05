use super::admin_shell::{admin_client, resolve_admin_token, AdminPageState, AdminShell};
use leptos::prelude::*;
use leptos_router::hooks::query_signal;
use web_sdk::AdminUsageStats;

fn period_label(period: &str) -> &'static str {
    match period {
        "7d" => "近 7 天",
        "90d" => "近 90 天",
        _ => "近 30 天",
    }
}

#[component]
pub fn AdminUsagePage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let (owner_query, set_owner_query) = query_signal::<String>("owner");
    let (period_query, set_period_query) = query_signal::<String>("period");

    let stats = RwSignal::new(None::<AdminUsageStats>);
    let owner_input = RwSignal::new(String::new());

    let fetch_usage = move |owner: String, period: String| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() || owner.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_usage(&owner, &period).await {
                Ok(usage) => {
                    stats.set(Some(usage));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    };

    Effect::new(move |_| {
        let owner = owner_query.get().filter(|o| !o.is_empty());
        let period = period_query.get().unwrap_or_else(|| "30d".to_string());
        if let Some(owner) = owner {
            owner_input.set(owner.clone());
            fetch_usage(owner, period);
        } else {
            state.set_ready();
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let owner = owner_input.get_untracked().trim().to_string();
        if owner.is_empty() {
            state.set_error("请输入要查询的账户 ID (owner_user_id)。");
            return;
        }
        let period = period_query.get_untracked().unwrap_or_else(|| "30d".to_string());
        set_owner_query.set(Some(owner.clone()));
        set_period_query.set(Some(period.clone()));
        fetch_usage(owner, period);
    };

    view! {
        <AdminShell title="用量监控" test_id="admin-usage-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"账户模型用量"</h2>
                </header>
                <form class="admin-filter-form" on:submit=on_submit>
                    <input
                        type="text"
                        class="admin-filter-input"
                        data-testid="admin-usage-owner"
                        placeholder="账户 ID (owner_user_id)"
                        prop:value=move || owner_input.get()
                        on:input=move |ev| owner_input.set(event_target_value(&ev))
                    />
                    <select
                        class="admin-filter-input"
                        data-testid="admin-usage-period"
                        prop:value=move || period_query.get().unwrap_or_else(|| "30d".to_string())
                        on:change=move |ev| set_period_query.set(Some(event_target_value(&ev)))
                    >
                        <option value="7d">"近 7 天"</option>
                        <option value="30d">"近 30 天"</option>
                        <option value="90d">"近 90 天"</option>
                    </select>
                    <button type="submit" class="admin-panel-btn" data-testid="admin-usage-submit">
                        "查询用量"
                    </button>
                </form>
                <Show when=move || owner_input.get_untracked().is_empty()>
                    <p class="admin-empty" data-testid="admin-usage-empty">
                        "输入账户 ID 并选择统计周期后查询；也可以从账户详情页进入。"
                    </p>
                </Show>
                <Show when=move || stats.get().is_some()>
                    <div class="admin-stat-list" data-testid="admin-usage-stats">
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"统计周期"</span>
                            <span class="admin-stat-value">
                                {move || {
                                    stats.get()
                                        .map(|s| period_label(&s.period).to_string())
                                        .unwrap_or_default()
                                }}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"查询次数"</span>
                            <span class="admin-stat-value">
                                {move || stats.get().map(|s| s.query_count).unwrap_or(0)}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"资料数"</span>
                            <span class="admin-stat-value">
                                {move || stats.get().map(|s| s.document_count).unwrap_or(0)}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"切片数"</span>
                            <span class="admin-stat-value">
                                {move || stats.get().map(|s| s.chunk_count).unwrap_or(0)}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"存储字节数"</span>
                            <span class="admin-stat-value">
                                {move || stats.get().map(|s| s.storage_bytes).unwrap_or(0)}
                            </span>
                        </div>
                    </div>
                </Show>
            </section>
        </AdminShell>
    }
}
