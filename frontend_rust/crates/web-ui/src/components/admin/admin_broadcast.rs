use super::admin_shell::{admin_client, resolve_admin_token, AdminPageState, AdminShell};
use leptos::prelude::*;
use web_sdk::AdminBroadcastRequest;

#[component]
pub fn AdminBroadcastPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);

    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let event_type = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let created = RwSignal::new(None::<i64>);

    // 首次 token 校验后即可显示表单（无初始拉取）。
    Effect::new(move |_| {
        if resolve_admin_token(&token).is_empty() {
            return;
        }
        state.set_ready();
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let title_val = title.get_untracked().trim().to_string();
        let body_val = body.get_untracked().trim().to_string();
        let event_val = event_type.get_untracked().trim().to_string();
        if title_val.is_empty() || body_val.is_empty() {
            state.set_error("标题与正文均为必填。");
            return;
        }
        let tok = resolve_admin_token(&token);
        if tok.is_empty() || busy.get_untracked() {
            return;
        }
        let req = AdminBroadcastRequest {
            event_type: if event_val.is_empty() { None } else { Some(event_val) },
            title: title_val,
            body: body_val,
            data: None,
        };
        busy.set(true);
        created.set(None);
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).broadcast_admin_notification(&req).await {
                Ok(result) => {
                    created.set(Some(result.created));
                    title.set(String::new());
                    body.set(String::new());
                    event_type.set(String::new());
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
            busy.set(false);
        });
    };

    view! {
        <AdminShell title="公告广播" test_id="admin-broadcast-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"全平台系统公告广播"</h2>
                    <p class="admin-panel-desc">
                        "公告通过通知通道下发给在线用户；event_type 留空时使用默认 admin.broadcast。"
                    </p>
                </header>
                <form class="admin-form" on:submit=on_submit>
                    <label class="admin-field">
                        <span class="admin-field-label">"公告标题"</span>
                        <input
                            type="text"
                            data-testid="admin-broadcast-title"
                            placeholder="例如：服务升级通知"
                            prop:value=move || title.get()
                            on:input=move |ev| title.set(event_target_value(&ev))
                        />
                    </label>
                    <label class="admin-field">
                        <span class="admin-field-label">"公告正文"</span>
                        <textarea
                            rows="4"
                            data-testid="admin-broadcast-body"
                            placeholder="公告内容…"
                            prop:value=move || body.get()
                            on:input=move |ev| body.set(event_target_value(&ev))
                        ></textarea>
                    </label>
                    <label class="admin-field">
                        <span class="admin-field-label">"事件类型 (可选)"</span>
                        <input
                            type="text"
                            data-testid="admin-broadcast-event-type"
                            placeholder="admin.broadcast"
                            prop:value=move || event_type.get()
                            on:input=move |ev| event_type.set(event_target_value(&ev))
                        />
                    </label>
                    <button
                        type="submit"
                        class="admin-panel-btn"
                        data-testid="admin-broadcast-submit"
                        disabled=move || busy.get()
                    >
                        {move || if busy.get() { "发送中…" } else { "发送公告" }}
                    </button>
                </form>
                {move || {
                    created.get().map(|count| {
                        view! {
                            <p class="admin-success" data-testid="admin-broadcast-result">
                                {format!("公告已创建，共送达 {count} 个通知端。")}
                            </p>
                        }
                    })
                }}
            </section>
        </AdminShell>
    }
}
