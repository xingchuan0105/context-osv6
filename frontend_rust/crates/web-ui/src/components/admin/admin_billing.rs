use super::admin_shell::{admin_client, resolve_admin_token, AdminPageState, AdminShell};
use super::admin_status_pages::AdminStatList;
use leptos::prelude::*;
use web_sdk::AdminBillingOverview;

#[component]
pub fn AdminBillingPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let billing = RwSignal::new(None::<AdminBillingOverview>);

    Effect::new(move |_| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).get_admin_billing_overview().await {
                Ok(value) => {
                    billing.set(Some(value));
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    });

    let rows = Signal::derive(move || {
        billing.get()
            .map(|b| {
                vec![
                    ("生效订阅", b.active_subscriptions.to_string()),
                    ("逾期订阅", b.past_due_subscriptions.to_string()),
                    ("未支付订阅", b.unpaid_subscriptions.to_string()),
                    ("已取消订阅", b.canceled_subscriptions.to_string()),
                ]
            })
            .unwrap_or_default()
    });

    view! {
        <AdminShell title="平台计费" test_id="admin-billing-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"全平台计费账单概览"</h2>
                </header>
                <AdminStatList rows=rows list_test_id="admin-billing-stats" empty_test_id="admin-billing-empty"/>
            </section>
        </AdminShell>
    }
}
