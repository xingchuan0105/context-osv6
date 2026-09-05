use super::admin_shell::{
    admin_client, resolve_admin_token, AdminGateState, AdminPageState, AdminShell,
};
use leptos::prelude::*;
use web_sdk::{AdminFeatureFlagChangeRequest, AdminFeatureFlagEntry};

#[component]
pub fn AdminFeatureFlagsPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);

    let flags = RwSignal::new(Vec::<AdminFeatureFlagEntry>::new());
    let requests = RwSignal::new(Vec::<AdminFeatureFlagChangeRequest>::new());

    let req_key = RwSignal::new(String::new());
    let req_enabled = RwSignal::new(true);
    let req_reason = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let result = RwSignal::new(None::<String>);

    let refresh = move || {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        let state = state.clone();
        leptos::task::spawn_local(async move {
            let client = admin_client(&tok);
            match client.list_admin_feature_flags().await {
                Ok(list) => flags.set(list),
                Err(err) => {
                    state.report_error(&err);
                    return;
                }
            }
            match client.list_admin_feature_flag_change_requests(None).await {
                Ok(list) => requests.set(list),
                Err(err) => {
                    state.report_error(&err);
                    return;
                }
            }
            state.set_ready();
        });
    };

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        refresh();
    });

    let on_create_request = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let key = req_key.get_untracked().trim().to_string();
        let reason = req_reason.get_untracked().trim().to_string();
        let enabled = req_enabled.get_untracked();
        if key.is_empty() || reason.is_empty() {
            state.set_error("开关 Key 与变更理由均为必填。");
            return;
        }
        let tok = resolve_admin_token(&token);
        if tok.is_empty() || busy.get_untracked() {
            return;
        }
        busy.set(true);
        result.set(None);
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok)
                .create_admin_feature_flag_change_request(&key, enabled, &reason)
                .await
            {
                Ok(request) => {
                    result.set(Some(format!(
                        "变更请求 {} 已提交，等待复核。",
                        request.id
                    )));
                    req_key.set(String::new());
                    req_reason.set(String::new());
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
            busy.set(false);
            refresh();
        });
    };

    let on_review = move |request_id: String, approved: bool| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() || busy.get_untracked() {
            return;
        }
        busy.set(true);
        let state = state.clone();
        leptos::task::spawn_local(async move {
            if let Err(err) = admin_client(&tok)
                .review_admin_feature_flag_change_request(&request_id, approved, None)
                .await
            {
                state.report_error(&err);
            }
            busy.set(false);
            refresh();
        });
    };

    view! {
        <AdminShell title="功能开关" test_id="admin-flags-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"功能灰度开关"</h2>
                    <span class="admin-panel-meta" data-testid="admin-flags-total">
                        {move || format!("{} 个开关", flags.get().len())}
                    </span>
                </header>
                <Show
                    when=move || state.gate.get() == AdminGateState::Ready && flags.get().is_empty()
                >
                    <p class="admin-empty" data-testid="admin-flags-empty">
                        "暂无功能开关配置。"
                    </p>
                </Show>
                <div class="admin-table-wrap" data-testid="admin-flags-table">
                    <table class="admin-table">
                        <thead>
                            <tr>
                                <th>"Key"</th>
                                <th>"分类"</th>
                                <th>"说明"</th>
                                <th>"已启用"</th>
                                <th>"生效中"</th>
                                <th>"待复核"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || flags.get()
                                key=|flag| flag.key.clone()
                                children=move |flag| {
                                    let enabled = if flag.enabled { "是" } else { "否" };
                                    let effective = if flag.effective_enabled { "是" } else { "否" };
                                    let pending = if flag.has_pending_request { "有" } else { "无" };
                                    view! {
                                        <tr class="admin-table-row" data-testid="admin-flag-row">
                                            <td class="admin-table-mono">{flag.key}</td>
                                            <td>{flag.category}</td>
                                            <td>{flag.description}</td>
                                            <td>{enabled}</td>
                                            <td>{effective}</td>
                                            <td>{pending}</td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
            </section>

            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"变更请求"</h2>
                </header>
                <Show
                    when=move || state.gate.get() == AdminGateState::Ready && requests.get().is_empty()
                >
                    <p class="admin-empty" data-testid="admin-flag-requests-empty">
                        "暂无变更请求。"
                    </p>
                </Show>
                <div class="admin-table-wrap" data-testid="admin-flag-requests">
                    <table class="admin-table">
                        <thead>
                            <tr>
                                <th>"开关"</th>
                                <th>"当前"</th>
                                <th>"申请"</th>
                                <th>"理由"</th>
                                <th>"状态"</th>
                                <th>"复核"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || requests.get()
                                key=|req| req.id.clone()
                                children=move |req| {
                                    let req_id = req.id.clone();
                                    let req_id_reject = req.id.clone();
                                    let review_cell = if req.status == "pending" {
                                        view! {
                                            <span class="admin-actions">
                                                <button
                                                    type="button"
                                                    class="admin-table-link"
                                                    data-testid=format!("admin-flag-approve-{}", req.id)
                                                    on:click=move |_| on_review(req_id.clone(), true)
                                                >
                                                    "通过"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="admin-table-danger"
                                                    data-testid=format!("admin-flag-reject-{}", req.id)
                                                    on:click=move |_| on_review(req_id_reject.clone(), false)
                                                >
                                                    "驳回"
                                                </button>
                                            </span>
                                        }
                                            .into_any()
                                    } else {
                                        view! { <span>{req.status.clone()}</span> }.into_any()
                                    };
                                    view! {
                                        <tr class="admin-table-row" data-testid="admin-flag-request-row">
                                            <td class="admin-table-mono">{req.flag_key}</td>
                                            <td>{if req.current_enabled { "开" } else { "关" }}</td>
                                            <td>{if req.requested_enabled { "开" } else { "关" }}</td>
                                            <td>{req.reason}</td>
                                            <td>{req.status}</td>
                                            <td>{review_cell}</td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
            </section>

            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"提交变更请求"</h2>
                    <p class="admin-panel-desc">
                        "开关状态不直接改写；提交请求后由管理员复核执行。"
                    </p>
                </header>
                <form class="admin-form" on:submit=on_create_request>
                    <label class="admin-field">
                        <span class="admin-field-label">"开关 Key"</span>
                        <input
                            type="text"
                            data-testid="admin-flag-req-key"
                            placeholder="例如：rag.offline"
                            prop:value=move || req_key.get()
                            on:input=move |ev| req_key.set(event_target_value(&ev))
                        />
                    </label>
                    <label class="admin-field">
                        <span class="admin-field-label">"目标状态"</span>
                        <select
                            data-testid="admin-flag-req-enabled"
                            prop:value=move || {
                                if req_enabled.get() { "on".to_string() } else { "off".to_string() }
                            }
                            on:change=move |ev| req_enabled.set(event_target_value(&ev) == "on")
                        >
                            <option value="on">"开启"</option>
                            <option value="off">"关闭"</option>
                        </select>
                    </label>
                    <label class="admin-field">
                        <span class="admin-field-label">"变更理由"</span>
                        <input
                            type="text"
                            data-testid="admin-flag-req-reason"
                            placeholder="说明变更原因"
                            prop:value=move || req_reason.get()
                            on:input=move |ev| req_reason.set(event_target_value(&ev))
                        />
                    </label>
                    <button
                        type="submit"
                        class="admin-panel-btn"
                        data-testid="admin-flag-req-submit"
                        disabled=move || busy.get()
                    >
                        "提交请求"
                    </button>
                </form>
                {move || {
                    result.get().map(|msg| {
                        view! {
                            <p class="admin-success" data-testid="admin-flag-req-result">{msg}</p>
                        }
                    })
                }}
            </section>
        </AdminShell>
    }
}
