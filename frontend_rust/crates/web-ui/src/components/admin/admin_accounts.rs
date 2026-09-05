use super::admin_shell::{admin_client, resolve_admin_token, AdminGateState, AdminPageState, AdminShell};
use leptos::prelude::*;
use leptos_router::hooks::use_params;
use leptos_router::params::Params;
use web_sdk::AdminAccountInfo;

#[derive(Params, PartialEq, Clone, Debug)]
struct AccountDetailParams {
    owner_user_id: Option<String>,
}

const ACCOUNTS_PAGE_SIZE: usize = 10;

/// epoch 秒 → `YYYY-MM-DD`（Howard Hinnant civil_from_days 算法，无额外依赖）。
pub fn fmt_epoch_date(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }
    let z = ts.div_euclid(86_400) + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}

#[component]
pub fn AdminAccountsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);

    let accounts = RwSignal::new(Vec::<AdminAccountInfo>::new());
    let page = RwSignal::new(0usize);

    let total_pages = Signal::derive(move || {
        accounts
            .with(|list| list.len().div_ceil(ACCOUNTS_PAGE_SIZE))
            .max(1)
    });
    let prev_disabled = Signal::derive(move || page.get() == 0);
    let next_disabled = Signal::derive(move || page.get() + 1 >= total_pages.get());
    let paged = Signal::derive(move || {
        let start = page.get() * ACCOUNTS_PAGE_SIZE;
        accounts
            .get()
            .into_iter()
            .skip(start)
            .take(ACCOUNTS_PAGE_SIZE)
            .collect::<Vec<_>>()
    });

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).list_admin_accounts(1, 500).await {
                Ok(list) => {
                    accounts.set(list);
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    view! {
        <AdminShell title="账户管理" test_id="admin-accounts-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"全部账户"</h2>
                    <span class="admin-panel-meta" data-testid="admin-accounts-total">
                        {move || format!("共 {} 个账户", accounts.get().len())}
                    </span>
                </header>
                <Show
                    when=move || state.gate.get() == AdminGateState::Ready && accounts.get().is_empty()
                >
                    <p class="admin-empty" data-testid="admin-accounts-empty">
                        "暂无账户。新注册的账户将出现在这里。"
                    </p>
                </Show>
                <div class="admin-table-wrap" data-testid="admin-accounts-table">
                    <table class="admin-table">
                        <thead>
                            <tr>
                                <th>"账户名"</th>
                                <th>"账户 ID"</th>
                                <th>"用户数"</th>
                                <th>"资料数"</th>
                                <th>"查询数"</th>
                                <th>"状态"</th>
                                <th>"操作"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || paged.get()
                                key=|acc| acc.id.clone()
                                children=move |acc| {
                                    let detail_href = format!("/admin/accounts/{}", acc.id);
                                    let status = if acc.blocked { "已封禁" } else { "正常" };
                                    view! {
                                        <tr class="admin-table-row" data-testid="admin-account-row">
                                            <td>{acc.name}</td>
                                            <td class="admin-table-mono">{acc.id.clone()}</td>
                                            <td>{acc.user_count}</td>
                                            <td>{acc.document_count}</td>
                                            <td>{acc.query_count}</td>
                                            <td>{status}</td>
                                            <td>
                                                <a
                                                    href=detail_href
                                                    class="admin-table-link"
                                                    data-testid="admin-account-detail-link"
                                                >
                                                    "详情"
                                                </a>
                                            </td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
                <div class="admin-pager" data-testid="admin-accounts-pager">
                    <button
                        type="button"
                        class="admin-pager-btn"
                        data-testid="admin-accounts-prev"
                        disabled=move || prev_disabled.get()
                        on:click=move |_| page.update(|p| *p = p.saturating_sub(1))
                    >
                        "上一页"
                    </button>
                    <span class="admin-pager-label" data-testid="admin-accounts-page-label">
                        {move || format!("第 {} / {} 页", page.get() + 1, total_pages.get())}
                    </span>
                    <button
                        type="button"
                        class="admin-pager-btn"
                        data-testid="admin-accounts-next"
                        disabled=move || next_disabled.get()
                        on:click=move |_| page.update(|p| *p += 1)
                    >
                        "下一页"
                    </button>
                </div>
            </section>
        </AdminShell>
    }
}

#[component]
pub fn AdminAccountDetailPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let params = use_params::<AccountDetailParams>();
    let owner_from_path = Signal::derive(move || {
        params
            .read()
            .as_ref()
            .ok()
            .and_then(|p| p.owner_user_id.clone())
            .unwrap_or_default()
    });

    let account = RwSignal::new(None::<AdminAccountInfo>);
    let busy = RwSignal::new(false);
    let owner = RwSignal::new(String::new());

    Effect::new(move |_| {
        if owner.get_untracked().is_empty() {
            let id = owner_from_path.get_untracked();
            if !id.is_empty() {
                owner.set(id);
            }
        }
        let wid = owner.get();
        if wid.is_empty() {
            return;
        }
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_account(&wid).await {
                Ok(acc) => {
                    account.set(Some(acc));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    let on_toggle_block = move |_| {
        let Some(acc) = account.get_untracked() else {
            return;
        };
        let tok = resolve_admin_token(&token);
        if tok.is_empty() || busy.get_untracked() {
            return;
        }
        let target = !acc.blocked;
        busy.set(true);
        let state = state.clone();
        leptos::task::spawn_local(async move {
            let client = admin_client(&tok);
            match client.block_admin_account(&acc.id, target).await {
                Ok(()) => match client.get_admin_account(&acc.id).await {
                    Ok(refreshed) => account.set(Some(refreshed)),
                    Err(err) => state.report_error(&err),
                },
                Err(err) => state.report_error(&err),
            }
            busy.set(false);
        });
    };

    let users_href = Signal::derive(move || {
        let id = account.get().map(|a| a.id).unwrap_or_default();
        format!("/admin/users?owner={id}")
    });
    let usage_href = Signal::derive(move || {
        let id = account.get().map(|a| a.id).unwrap_or_default();
        format!("/admin/usage?owner={id}&period=30d")
    });

    view! {
        <AdminShell title="账户详情" test_id="admin-account-detail">
            <Show when=move || account.get().is_some()>
                <section class="admin-panel">
                    <header class="admin-panel-header">
                        <h2 class="admin-panel-title" data-testid="admin-account-name">
                            {move || account.get().map(|a| a.name).unwrap_or_default()}
                        </h2>
                        <span class="admin-panel-meta admin-table-mono">
                            {move || account.get().map(|a| a.id.clone()).unwrap_or_default()}
                        </span>
                    </header>
                    <div class="admin-stat-list" data-testid="admin-account-stats">
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"用户数"</span>
                            <span class="admin-stat-value">
                                {move || account.get().map(|a| a.user_count).unwrap_or(0)}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"资料数"</span>
                            <span class="admin-stat-value">
                                {move || account.get().map(|a| a.document_count).unwrap_or(0)}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"查询数"</span>
                            <span class="admin-stat-value">
                                {move || account.get().map(|a| a.query_count).unwrap_or(0)}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"创建日期"</span>
                            <span class="admin-stat-value">
                                {move || account.get().map(|a| fmt_epoch_date(a.created_at)).unwrap_or_default()}
                            </span>
                        </div>
                        <div class="admin-stat-row">
                            <span class="admin-stat-label">"状态"</span>
                            <span class="admin-stat-value" data-testid="admin-account-status">
                                {move || {
                                    if account.get().map(|a| a.blocked).unwrap_or(false) {
                                        "已封禁"
                                    } else {
                                        "正常"
                                    }
                                }}
                            </span>
                        </div>
                    </div>
                    <div class="admin-actions">
                        <button
                            type="button"
                            class="admin-panel-btn"
                            data-testid="admin-account-block-btn"
                            disabled=move || busy.get()
                            on:click=on_toggle_block
                        >
                            {move || {
                                if account.get().map(|a| a.blocked).unwrap_or(false) {
                                    "解除封禁"
                                } else {
                                    "封禁账户"
                                }
                            }}
                        </button>
                        <a href=users_href class="admin-panel-btn" data-testid="admin-account-users-link">
                            "查看用户"
                        </a>
                        <a href=usage_href class="admin-panel-btn" data-testid="admin-account-usage-link">
                            "查询用量"
                        </a>
                    </div>
                </section>
            </Show>
        </AdminShell>
    }
}
