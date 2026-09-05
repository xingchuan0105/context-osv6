use super::admin_accounts::fmt_epoch_date;
use super::admin_shell::{
    admin_client, resolve_admin_token, AdminGateState, AdminPageState, AdminShell,
};
use leptos::prelude::*;
use leptos_router::hooks::query_signal;
use web_sdk::AdminUserInfo;

fn role_label(role: &str) -> String {
    match role {
        "super_admin" => "超级管理员".to_string(),
        "ops_admin" => "运维管理员".to_string(),
        "finance_admin" => "财务管理员".to_string(),
        other => other.to_string(),
    }
}

#[component]
pub fn AdminUsersPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let state = AdminPageState::new();
    provide_context(state);
    let (owner_query, set_owner_query) = query_signal::<String>("owner");

    let users = RwSignal::new(Vec::<AdminUserInfo>::new());
    let owner_input = RwSignal::new(String::new());
    let confirm_id = RwSignal::new(None::<String>);

    let fetch_users = move |owner: String| {
        let tok = resolve_admin_token(&token);
        if tok.is_empty() || owner.is_empty() {
            return;
        }
        state.set_loading();
        let state = state.clone();
        leptos::task::spawn_local(async move {
            match admin_client(&tok).list_admin_users(&owner).await {
                Ok(list) => {
                    users.set(list);
                    state.set_ready();
                }
                Err(err) => state.report_error(&err),
            }
        });
    };

    Effect::new(move |_| {
        if let Some(owner) = owner_query.get().filter(|o| !o.is_empty()) {
            owner_input.set(owner.clone());
            fetch_users(owner);
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
        set_owner_query.set(Some(owner.clone()));
        fetch_users(owner);
    };

    let on_delete = move |user_id: String| {
        if confirm_id.get_untracked().as_deref() != Some(user_id.as_str()) {
            confirm_id.set(Some(user_id));
            return;
        }
        let tok = resolve_admin_token(&token);
        if tok.is_empty() {
            return;
        }
        let owner = owner_input.get_untracked().trim().to_string();
        confirm_id.set(None);
        let state = state.clone();
        leptos::task::spawn_local(async move {
            if let Err(err) = admin_client(&tok).delete_admin_user(&user_id).await {
                state.report_error(&err);
                return;
            }
            if owner.is_empty() {
                return;
            }
            match admin_client(&tok).list_admin_users(&owner).await {
                Ok(list) => users.set(list),
                Err(err) => state.report_error(&err),
            }
        });
    };

    view! {
        <AdminShell title="用户管理" test_id="admin-users-page">
            <section class="admin-panel">
                <header class="admin-panel-header">
                    <h2 class="admin-panel-title">"账户内用户列表"</h2>
                </header>
                <form class="admin-filter-form" on:submit=on_submit>
                    <input
                        type="text"
                        class="admin-filter-input"
                        data-testid="admin-owner-input"
                        placeholder="输入账户 ID (owner_user_id)"
                        prop:value=move || owner_input.get()
                        on:input=move |ev| owner_input.set(event_target_value(&ev))
                    />
                    <button type="submit" class="admin-panel-btn" data-testid="admin-owner-submit">
                        "查询用户"
                    </button>
                </form>
                <Show
                    when=move || {
                        state.gate.get() == AdminGateState::Ready
                            && !owner_input.get_untracked().is_empty()
                            && users.get().is_empty()
                    }
                >
                    <p class="admin-empty" data-testid="admin-users-empty">
                        "该账户下暂无用户，或账户 ID 无效。"
                    </p>
                </Show>
                <Show when=move || owner_input.get_untracked().is_empty()>
                    <p class="admin-empty" data-testid="admin-users-no-owner">
                        "请输入账户 ID 查询该账户下的用户；也可以从账户详情页进入。"
                    </p>
                </Show>
                <div class="admin-table-wrap" data-testid="admin-users-table">
                    <table class="admin-table">
                        <thead>
                            <tr>
                                <th>"邮箱"</th>
                                <th>"角色"</th>
                                <th>"用户 ID"</th>
                                <th>"创建日期"</th>
                                <th>"操作"</th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || users.get()
                                key=|user| user.id.clone()
                                children=move |user| {
                                    let user_id = user.id.clone();
                                    let user_id2 = user.id.clone();
                                    let user_id_label = user.id.clone();
                                    view! {
                                        <tr class="admin-table-row" data-testid="admin-user-row">
                                            <td>{user.email}</td>
                                            <td>{role_label(&user.role)}</td>
                                            <td class="admin-table-mono">{user_id}</td>
                                            <td>{fmt_epoch_date(user.created_at)}</td>
                                            <td>
                                                <button
                                                    type="button"
                                                    class="admin-table-danger"
                                                    data-testid=format!("admin-user-delete-{}", user_id2)
                                                    on:click=move |_| on_delete(user_id2.clone())
                                                >
                                                    {move || {
                                                        if confirm_id.get().as_deref() == Some(user_id_label.as_str()) {
                                                            "确认删除"
                                                        } else {
                                                            "删除"
                                                        }
                                                    }}
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </div>
            </section>
        </AdminShell>
    }
}
