use super::admin_shell::{
    admin_client, resolve_admin_token, AdminGateState, AdminPageState, AdminShell,
};
use leptos::prelude::*;
use web_sdk::{AdminAuditLogEntry, AdminAuditLogPage, AdminAuditLogQuery};

const AUDIT_PAGE_SIZE: usize = 20;

fn window_options() -> &'static [(&'static str, &'static str)] {
    &[("24h", "近 24 小时"), ("7d", "近 7 天"), ("30d", "近 30 天"), ("90d", "近 90 天")]
}

#[cfg(target_arch = "wasm32")]
fn trigger_csv_download(bytes: &[u8], filename: &str) {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let Ok(blob) = web_sys::Blob::new_with_u8_array_sequence(&parts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let anchor = match document.create_element("a") {
        Ok(element) => match element.dyn_into::<web_sys::HtmlElement>() {
            Ok(anchor) => anchor,
            Err(_) => {
                let _ = web_sys::Url::revoke_object_url(&url);
                return;
            }
        },
        Err(_) => {
            let _ = web_sys::Url::revoke_object_url(&url);
            return;
        }
    };
    let _ = anchor.set_attribute("href", &url);
    let _ = anchor.set_attribute("download", filename);
    if let Some(body) = document.body() {
        let _ = body.append_child(&anchor);
        anchor.click();
        let _ = body.remove_child(&anchor);
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

#[cfg(not(target_arch = "wasm32"))]
fn trigger_csv_download(_bytes: &[u8], _filename: &str) {}

#[component]
pub fn AdminAuditLogsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);

    let filter_query = RwSignal::new(String::new());
    let filter_action = RwSignal::new(String::new());
    let filter_resource = RwSignal::new(String::new());
    let filter_actor = RwSignal::new(String::new());
    let filter_window = RwSignal::new(String::new());
    let applied = RwSignal::new(AdminAuditLogQuery {
        page: Some(1),
        per_page: Some(AUDIT_PAGE_SIZE),
        ..AdminAuditLogQuery::default()
    });
    let logs = RwSignal::new(AdminAuditLogPage {
        items: Vec::new(),
        total: 0,
        page: 1,
        per_page: AUDIT_PAGE_SIZE,
    });

    let fetch_logs = move |query: AdminAuditLogQuery| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).list_admin_audit_logs(&query).await {
                Ok(page) => {
                    logs.set(page);
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    };

    Effect::new(move |_| {
        let query = applied.get();
        fetch_logs(query);
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let opt = |sig: RwSignal<String>| {
            let value = sig.get_untracked().trim().to_string();
            if value.is_empty() { None } else { Some(value) }
        };
        applied.set(AdminAuditLogQuery {
            query: opt(filter_query),
            action: opt(filter_action),
            resource_type: opt(filter_resource),
            actor: opt(filter_actor),
            window: opt(filter_window),
            page: Some(1),
            per_page: Some(AUDIT_PAGE_SIZE),
        });
    };

    let on_prev = move |_| {
        applied.update(|q| q.page = Some(q.page.unwrap_or(1).saturating_sub(1).max(1)));
    };
    let prev_disabled = Signal::derive(move || applied.get().page.unwrap_or(1) <= 1);
    let on_next = move |_| {
        applied.update(|q| {
            let total = logs.get_untracked().total;
            let max_page = total.div_ceil(q.per_page.unwrap_or(AUDIT_PAGE_SIZE).max(1)).max(1);
            let current = q.page.unwrap_or(1);
            if current < max_page {
                q.page = Some(current + 1);
            }
        });
    };

    let on_export = move |_| {
        let query = applied.get_untracked();
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_audit_logs_csv(&query).await {
                Ok(bytes) => trigger_csv_download(&bytes, "audit-logs.csv"),
                Err(err) => state.set_error(format!("导出失败：{err}")),
            }
        });
    };

    view! {
        <AdminShell title="审计日志" test_id="admin-audit-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"管理操作审计日志"</h2>
                    <span class="admin-panel-meta" data-testid="admin-audit-total">
                        {move || {
                            let q = applied.get();
                            let total = logs.get().total;
                            let max_page = total.div_ceil(q.per_page.unwrap_or(AUDIT_PAGE_SIZE).max(1)).max(1);
                            format!("共 {} 条 · 第 {} / {} 页", total, q.page.unwrap_or(1), max_page)
                        }}
                    </span>
                </header>
                <form class="admin-filter-form" on:submit=on_submit>
                    <input
                        type="text"
                        class="admin-filter-input"
                        data-testid="admin-audit-filter-query"
                        placeholder="关键词"
                        prop:value=move || filter_query.get()
                        on:input=move |ev| filter_query.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="admin-filter-input"
                        data-testid="admin-audit-filter-action"
                        placeholder="操作 (action)"
                        prop:value=move || filter_action.get()
                        on:input=move |ev| filter_action.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="admin-filter-input"
                        data-testid="admin-audit-filter-resource"
                        placeholder="资源类型"
                        prop:value=move || filter_resource.get()
                        on:input=move |ev| filter_resource.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="admin-filter-input"
                        data-testid="admin-audit-filter-actor"
                        placeholder="操作者"
                        prop:value=move || filter_actor.get()
                        on:input=move |ev| filter_actor.set(event_target_value(&ev))
                    />
                    <select
                        class="admin-filter-input"
                        data-testid="admin-audit-filter-window"
                        prop:value=move || filter_window.get()
                        on:change=move |ev| filter_window.set(event_target_value(&ev))
                    >
                        <option value="">"全部时间"</option>
                        {window_options()
                            .iter()
                            .map(|(value, label)| {
                                view! { <option value=*value>{*label}</option> }
                            })
                            .collect_view()}
                    </select>
                    <button type="submit" class="admin-panel-btn" data-testid="admin-audit-submit">
                        "查询"
                    </button>
                    <button
                        type="button"
                        class="admin-panel-btn"
                        data-testid="admin-audit-csv"
                        on:click=on_export
                    >
                        "导出 CSV"
                    </button>
                </form>
                <Show
                    when=move || {
                        state.gate.get() == AdminGateState::Ready && logs.get().items.is_empty()
                    }
                >
                    <p class="admin-empty" data-testid="admin-audit-empty">
                        "当前筛选条件下暂无审计记录。"
                    </p>
                </Show>
                <div class="admin-table-wrap" data-testid="admin-audit-table">
                    <table class="admin-table">
                        <thead>
                            <tr>
                                <th>"ID"</th>
                                <th>"操作"</th>
                                <th>"资源类型"</th>
                                <th>"资源 ID"</th>
                                <th>"操作者"</th>
                                <th>"归属账户"</th>
                                <th>"时间"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || logs.get().items
                                key=|entry: &AdminAuditLogEntry| entry.id
                                children=move |entry| {
                                    view! {
                                        <tr class="admin-table-row" data-testid="admin-audit-row">
                                            <td>{entry.id}</td>
                                            <td>{entry.action}</td>
                                            <td>{entry.resource_type}</td>
                                            <td class="admin-table-mono">{entry.resource_id}</td>
                                            <td class="admin-table-mono">
                                                {entry.actor_id.unwrap_or_else(|| "-".to_string())}
                                            </td>
                                            <td class="admin-table-mono">
                                                {entry.owner_user_id.unwrap_or_else(|| "-".to_string())}
                                            </td>
                                            <td>{entry.created_at}</td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
                <div class="admin-pager" data-testid="admin-audit-pager">
                    <button
                        type="button"
                        class="admin-pager-btn"
                        data-testid="admin-audit-prev"
                        disabled=move || prev_disabled.get()
                        on:click=on_prev
                    >
                        "上一页"
                    </button>
                    <span class="admin-pager-label" data-testid="admin-audit-page-label">
                        {move || format!("第 {} 页", applied.get().page.unwrap_or(1))}
                    </span>
                    <button
                        type="button"
                        class="admin-pager-btn"
                        data-testid="admin-audit-next"
                        on:click=on_next
                    >
                        "下一页"
                    </button>
                </div>
            </section>
        </AdminShell>
    }
}
