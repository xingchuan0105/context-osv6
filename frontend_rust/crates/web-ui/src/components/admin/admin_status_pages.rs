use super::admin_shell::{admin_client, resolve_admin_token, AdminPageState, AdminShell};
use leptos::prelude::*;
use web_sdk::{AdminDegradationStatus, AdminHealthStatus, AdminRagHealthStatus, AdminWorkerStatus};

/// 只读状态行的统一渲染：`(标签, 值)` 列表。
#[component]
pub fn AdminStatList(
    rows: Signal<Vec<(&'static str, String)>>,
    list_test_id: &'static str,
    empty_test_id: &'static str,
) -> impl IntoView {
    view! {
        <Show when=move || rows.get().is_empty()>
            <p class="admin-empty" data-testid=empty_test_id>"暂无数据。"</p>
        </Show>
        <div class="admin-stat-list" data-testid=list_test_id>
            <For
                each=move || rows.get()
                key=|row| row.0
                children=move |row| {
                    view! {
                        <div class="admin-stat-row" data-testid="admin-stat-row">
                            <span class="admin-stat-label">{row.0}</span>
                            <span class="admin-stat-value">{row.1}</span>
                        </div>
                    }
                }
            />
        </div>
    }
}

#[component]
pub fn AdminHealthPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let health = RwSignal::new(None::<AdminHealthStatus>);

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_health().await {
                Ok(value) => {
                    health.set(Some(value));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    let rows = Signal::derive(move || {
        health.get()
            .map(|h| {
                vec![
                    ("服务状态", h.status),
                    ("版本", h.version),
                    ("运行时长 (秒)", h.uptime_secs.to_string()),
                ]
            })
            .unwrap_or_default()
    });

    view! {
        <AdminShell title="服务健康" test_id="admin-health-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"基础设施与服务健康检查"</h2>
                </header>
                <AdminStatList rows=rows list_test_id="admin-health-stats" empty_test_id="admin-health-empty"/>
            </section>
        </AdminShell>
    }
}

#[component]
pub fn AdminRagHealthPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let rag = RwSignal::new(None::<AdminRagHealthStatus>);

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_rag_health().await {
                Ok(value) => {
                    rag.set(Some(value));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    let rows = Signal::derive(move || {
        rag.get()
            .map(|r| {
                vec![
                    ("失败资料", r.failed_documents.to_string()),
                    ("排队任务", r.queued_tasks.to_string()),
                    ("处理中任务", r.processing_tasks.to_string()),
                    ("死信任务", r.dead_letter_tasks.to_string()),
                    ("近期护栏事件", r.recent_guard_events.to_string()),
                ]
            })
            .unwrap_or_default()
    });

    view! {
        <AdminShell title="RAG 健康" test_id="admin-rag-health-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"RAG 检索质量与降级指标"</h2>
                </header>
                <AdminStatList rows=rows list_test_id="admin-rag-health-stats" empty_test_id="admin-rag-health-empty"/>
            </section>
        </AdminShell>
    }
}

#[component]
pub fn AdminWorkersPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let workers = RwSignal::new(None::<AdminWorkerStatus>);

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_worker_status().await {
                Ok(value) => {
                    workers.set(Some(value));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    let rows = Signal::derive(move || {
        workers.get()
            .map(|w| {
                vec![
                    ("运行模式", w.runtime_mode),
                    ("排队任务", w.queued_tasks.to_string()),
                    ("处理中任务", w.processing_tasks.to_string()),
                    ("死信任务", w.dead_letter_tasks.to_string()),
                    ("失败资料", w.failed_documents.to_string()),
                ]
            })
            .unwrap_or_default()
    });

    view! {
        <AdminShell title="Workers 状态" test_id="admin-workers-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"后台异步队列与 Worker 节点"</h2>
                </header>
                <AdminStatList rows=rows list_test_id="admin-workers-stats" empty_test_id="admin-workers-empty"/>
            </section>
        </AdminShell>
    }
}

#[component]
pub fn AdminDegradationPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let degradation = RwSignal::new(None::<AdminDegradationStatus>);

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_degradation_status().await {
                Ok(value) => {
                    degradation.set(Some(value));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    let rows = Signal::derive(move || {
        degradation.get()
            .map(|d| {
                vec![
                    ("失败资料", d.failed_documents.to_string()),
                    ("近期护栏事件", d.recent_guard_events.to_string()),
                    ("分享访问事件", d.share_access_events.to_string()),
                ]
            })
            .unwrap_or_default()
    });

    view! {
        <AdminShell title="降级策略" test_id="admin-degradation-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"服务降级状态观测"</h2>
                </header>
                <AdminStatList rows=rows list_test_id="admin-degradation-stats" empty_test_id="admin-degradation-empty"/>
            </section>
        </AdminShell>
    }
}
