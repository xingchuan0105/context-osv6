use super::admin_shell::{resolve_admin_token, AdminPageState, AdminShell, ADMIN_NAV_ITEMS};
use leptos::prelude::*;

#[component]
pub fn AdminOverviewPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);

    Effect::new(move |_| {
        if resolve_admin_token(&token).is_empty() {
            return;
        }
        state.set_ready();
    });

    view! {
        <AdminShell title="管理后台" test_id="admin-overview">
            <section class="admin-panel" data-testid="admin-overview-grid">
                <p class="admin-panel-desc">
                    "平台运营控制台：账户、用户、用量、计费与系统运维的统一入口。"
                </p>
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
