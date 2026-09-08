use super::admin_shell::{admin_client, resolve_admin_token, AdminPageState, AdminShell, ADMIN_NAV_ITEMS};
use leptos::prelude::*;
use web_sdk::AdminAccountInfo;

#[component]
pub fn AdminOverviewPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let accounts = RwSignal::new(Vec::<AdminAccountInfo>::new());
    let query = RwSignal::new(String::new());
    let sort_blocked = RwSignal::new(false);

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).list_admin_accounts(1, 50).await {
                Ok(list) => {
                    accounts.set(list);
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    view! {
        <AdminShell title="管理后台" test_id="admin-overview">
            <section class="admin-panel" data-testid="admin-overview-grid">
                <p class="admin-panel-desc">
                    "平台运营控制台：账户、用户、用量、计费与系统运维的统一入口。"
                </p>
                <div class="settings-usage-cards" data-testid="admin-metric-cards">
                    <div class="settings-usage-card">
                        <span class="settings-usage-label">"账户数"</span>
                        <span class="settings-usage-value">{move || accounts.get().len().to_string()}</span>
                    </div>
                    <div class="settings-usage-card">
                        <span class="settings-usage-label">"已封禁"</span>
                        <span class="settings-usage-value" data-testid="admin-blocked-count">
                            {move || accounts.get().iter().filter(|a| a.blocked).count().to_string()}
                        </span>
                    </div>
                </div>
                <div class="admin-toolbar">
                    <input
                        type="search"
                        data-testid="admin-account-search"
                        placeholder="搜索账户"
                        prop:value=move || query.get()
                        on:input=move |ev| query.set(event_target_value(&ev))
                    />
                    <button type="button" data-testid="admin-sort-blocked" on:click=move |_| sort_blocked.update(|v| *v = !*v)>
                        "筛选封禁"
                    </button>
                </div>
                <ul class="admin-account-preview" data-testid="admin-account-preview">
                    {move || {
                        let q = query.get().to_lowercase();
                        let only_blocked = sort_blocked.get();
                        accounts
                            .get()
                            .into_iter()
                            .filter(|account| {
                                let matches = q.is_empty() || account.name.to_lowercase().contains(&q);
                                let blocked_ok = !only_blocked || account.blocked;
                                matches && blocked_ok
                            })
                            .take(8)
                            .map(|account| {
                                view! {
                                    <li data-testid="admin-account-row">
                                        <span>{account.name}</span>
                                        <span class=if account.blocked { "admin-badge is-blocked" } else { "admin-badge" }>
                                            {if account.blocked { "封禁" } else { "正常" }}
                                        </span>
                                    </li>
                                }
                            })
                            .collect_view()
                    }}
                </ul>
                <div class="admin-link-grid">
                    {ADMIN_NAV_ITEMS
                        .iter()
                        .filter(|entry| entry.0 != "overview")
                        .map(|(id, href, label)| {
                            let (id, href, label) = (*id, *href, *label);
                            view! {
                                <a href=href class="admin-link-card" data-testid=format!("admin-entry-{id}")>
                                    <span class="admin-link-card-label">{label}</span>
                                    <span class="admin-link-card-href">{href}</span>
                                </a>
                            }
                        })
                        .collect_view()}
                </div>
            </section>
        </AdminShell>
    }
}
