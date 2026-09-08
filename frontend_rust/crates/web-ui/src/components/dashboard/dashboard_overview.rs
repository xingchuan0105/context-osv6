use crate::api_base::poc_api_base;
use crate::components::shell::ProductChrome;
use crate::components::ui::{AppDialog, Toaster};
use crate::i18n::{tf_now, t_now, use_i18n};
use crate::routes::dest;
use contracts::workspaces::Workspace;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use web_sdk::BrowserRestClient;

#[component]
pub fn DashboardOverviewPage() -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let navigate = use_navigate();

    let workspaces = RwSignal::new(Vec::<Workspace>::new());
    let loading = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let show_create = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let new_desc = RwSignal::new(String::new());
    let create_loading = RwSignal::new(false);
    let tab = RwSignal::new("all".to_string());
    let sort_mode = RwSignal::new("recent".to_string());
    let view_mode = RwSignal::new("cards".to_string());
    let search = RwSignal::new(String::new());
    let show_search = RwSignal::new(false);
    let favorite_ids = RwSignal::new(Vec::<String>::new());
    let menu_id = RwSignal::new(None::<String>);
    let rename_target = RwSignal::new(None::<Workspace>);
    let delete_target = RwSignal::new(None::<Workspace>);
    let rename_name = RwSignal::new(String::new());
    let toaster = expect_context::<Toaster>();
    let i18n = use_i18n();

    let load_workspaces = move || {
        let tok = if token.get_untracked().is_empty() {
            web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
        } else {
            token.get_untracked()
        };
        if tok.is_empty() {
            return;
        }

        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
            match client.list_workspaces().await {
                Ok(resp) => {
                    workspaces.set(resp.workspaces);
                }
                Err(err) => {
                    error.set(Some(tf_now("dashboard.loadFailed", &[("error", &err.to_string())])));
                }
            }
            if let Ok(prefs) = client.get_preferences().await {
                favorite_ids.set(prefs.dashboard.favorite_workspace_ids);
            }
            loading.set(false);
        });
    };

    Effect::new(move |_| {
        load_workspaces();
    });

    let on_create = {
        let navigate = navigate.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let name_val = new_name.get().trim().to_string();
            let desc_val = new_desc.get().trim().to_string();
            if name_val.is_empty() {
                return;
            }
            let tok = if token.get_untracked().is_empty() {
                web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
            } else {
                token.get_untracked()
            };
            if tok.is_empty() || create_loading.get() {
                return;
            }

            create_loading.set(true);
            let navigate = navigate.clone();
            leptos::task::spawn_local(async move {
                let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
                match client.create_workspace(&name_val, &desc_val).await {
                    Ok(resp) => {
                        let wid = resp.workspace.id;
                        navigate(&format!("/dashboard/{wid}"), NavigateOptions::default());
                    }
                    Err(err) => {
                        error.set(Some(tf_now("dashboard.createFailed", &[("error", &err.to_string())])));
                    }
                }
                create_loading.set(false);
            });
        }
    };

    view! {
        <ProductChrome>
        <div class="dashboard-shell" data-testid="dashboard-overview">
            <header class="dashboard-header">
                <div class="dashboard-header-left">
                    <h1 class="dashboard-title">{move || i18n.t("dashboard.title")}</h1>
                </div>
                <div class="dashboard-header-actions">
                    <a href=dest::SHARE_TRAFFIC class="dashboard-header-btn">{move || i18n.t("dashboardShareTrafficNav")}</a>
                    <a href=dest::DESKTOP class="dashboard-header-btn" data-testid="dashboard-client-link">{move || i18n.t("productChrome.client")}</a>
                    <button
                        type="button"
                        class="dashboard-create-btn"
                        data-testid="create-workspace-btn"
                        on:click=move |_| show_create.set(true)
                    >
                        {move || i18n.t("dashboard.createWorkspace")}
                    </button>
                </div>
            </header>

            <div
                class="dashboard-modal-backdrop"
                hidden=move || !show_create.get()
            >
                <div class="dashboard-modal" role="dialog" aria-label=move || i18n.t("workspaceCreateDialogLabel")>
                    <header class="dashboard-modal-header">
                        <h2>{move || i18n.t("workspaceCreateDialogLabel")}</h2>
                        <button
                            type="button"
                            class="dashboard-modal-close"
                            on:click=move |_| show_create.set(false)
                        >
                            "×"
                        </button>
                    </header>
                    <form on:submit=on_create class="dashboard-modal-form">
                        <div class="auth-field">
                            <label for="ws-name">{move || i18n.t("dashboard.workspaceName")}</label>
                            <input
                                id="ws-name"
                                type="text"
                                data-testid="new-workspace-name"
                                placeholder=move || i18n.t("dashboard.workspaceNamePlaceholder")
                                prop:value=move || new_name.get()
                                on:input=move |ev| new_name.set(event_target_value(&ev))
                                required
                            />
                        </div>
                        <div class="auth-field">
                            <label for="ws-desc">{move || i18n.t("dashboard.workspaceDesc")}</label>
                            <input
                                id="ws-desc"
                                type="text"
                                data-testid="new-workspace-desc"
                                placeholder=move || i18n.t("dashboard.workspaceDescPlaceholder")
                                prop:value=move || new_desc.get()
                                on:input=move |ev| new_desc.set(event_target_value(&ev))
                            />
                        </div>
                        <div class="dashboard-modal-actions">
                            <button
                                type="button"
                                class="dashboard-btn-cancel"
                                on:click=move |_| show_create.set(false)
                            >
                                {move || i18n.t("commonCancel")}
                            </button>
                            <button
                                type="submit"
                                class="dashboard-btn-confirm"
                                data-testid="submit-create-workspace"
                                disabled=move || create_loading.get()
                            >
                                {move || {
                                    if create_loading.get() {
                                        i18n.t("dashboard.creating")
                                    } else {
                                        i18n.t("dashboard.confirmCreate")
                                    }
                                }}
                            </button>
                        </div>
                    </form>
                </div>
            </div>

            <nav class="dashboard-toolbar" aria-label=move || i18n.t("dashboard.filterLabel")>
                <button type="button" class=move || if tab.get() == "all" { "settings-nav-item is-active" } else { "settings-nav-item" } data-testid="dash-tab-all" on:click=move |_| tab.set("all".into())>{move || i18n.t("dashboardTabAll")}</button>
                <button type="button" class=move || if tab.get() == "mine" { "settings-nav-item is-active" } else { "settings-nav-item" } data-testid="dash-tab-mine" on:click=move |_| tab.set("mine".into())>{move || i18n.t("dashboard.tabMineShort")}</button>
                <button type="button" class=move || if tab.get() == "favorites" { "settings-nav-item is-active" } else { "settings-nav-item" } data-testid="dash-tab-favorites" on:click=move |_| tab.set("favorites".into())>{move || i18n.t("dashboard.tabFavoritesShort")}</button>
                <button type="button" class="dashboard-header-btn" data-testid="dash-sort" on:click=move |_| {
                    sort_mode.update(|mode| *mode = if *mode == "recent" { "name".into() } else { "recent".into() });
                }>{move || if sort_mode.get() == "name" { i18n.t("dashboard.sortByName") } else { i18n.t("dashboard.sortByRecent") }}</button>
                <button type="button" class="dashboard-header-btn" data-testid="dash-view" on:click=move |_| {
                    view_mode.update(|mode| *mode = if *mode == "cards" { "list".into() } else { "cards".into() });
                }>{move || if view_mode.get() == "list" { i18n.t("dashboardViewList") } else { i18n.t("dashboardViewCard") }}</button>
                <button type="button" class="dashboard-header-btn" data-testid="dash-search-open" on:click=move |_| show_search.set(true)>{move || i18n.t("dashboard.searchOpen")}</button>
            </nav>

            <AppDialog open=Signal::derive(move || show_search.get()) title_key="dashboardSearchDialogLabel" test_id="dashboard-search-dialog" on_close=Callback::new(move |_| show_search.set(false))>
                <input
                    type="search"
                    data-testid="dashboard-search-input"
                    placeholder=move || i18n.t("dashboard.searchNamePlaceholder")
                    prop:value=move || search.get()
                    on:input=move |ev| search.set(event_target_value(&ev))
                />
            </AppDialog>

            <main class="dashboard-content">
                {move || {
                    error.get().map(|msg| {
                        view! {
                            <p class="dashboard-error" role="alert" data-testid="page-error">
                                {msg}
                            </p>
                        }
                    })
                }}
                <Show when=move || loading.get()>
                    <p class="dashboard-loading" data-testid="page-loading">{move || i18n.t("dashboard.loadingList")}</p>
                </Show>
                <p class="page-status-empty" data-testid="page-empty" hidden=move || loading.get() || !visible_workspaces(workspaces.get(), tab.get(), favorite_ids.get(), search.get(), sort_mode.get()).is_empty()>
                    {move || i18n.t("dashboardEmptyAllTitle")}
                </p>
                <div
                    class=move || if view_mode.get() == "list" { "dashboard-workspace-list" } else { "dashboard-workspace-grid" }
                    data-testid="workspace-grid"
                >
                    <For
                        each=move || visible_workspaces(workspaces.get(), tab.get(), favorite_ids.get(), search.get(), sort_mode.get())
                        key=|ws| ws.id.clone()
                        children=move |ws| {
                            let wid = ws.id.clone();
                            let href = format!("/dashboard/{wid}");
                            let desc = if ws.description.is_empty() {
                                i18n.t("dashboardEmptyDescription")
                            } else {
                                ws.description.clone()
                            };
                            let doc_count = ws.document_count.to_string();
                            let docs_label = i18n.tf("dashboard.docCount", &[("count", doc_count.as_str())]);
                            let item = ws.clone();
                            let item_fav = item.clone();
                            let item_rename = item.clone();
                            let item_delete = item.clone();
                            let item_id = item.id.clone();
                            view! {
                                <article class="dashboard-workspace-card" data-testid="workspace-card">
                                    <a href=href.clone() class="dashboard-card-link">
                                        <div class="dashboard-card-top">
                                            <h3 class="dashboard-card-title">{ws.name.clone()}</h3>
                                            <span class="dashboard-card-docs">
                                                {docs_label}
                                            </span>
                                        </div>
                                        <p class="dashboard-card-desc">{desc}</p>
                                    </a>
                                    <button
                                        type="button"
                                        class="dashboard-card-menu"
                                        data-testid="workspace-menu"
                                        on:click=move |_| menu_id.set(Some(wid.clone()))
                                    >
                                        {move || i18n.t("commonMore")}
                                    </button>
                                    <div class="dashboard-card-actions" hidden=move || menu_id.get().as_deref() != Some(item_id.as_str())>
                                        <button type="button" data-testid="workspace-favorite" on:click=move |_| toggle_favorite(token, favorite_ids, toaster, item_fav.id.clone())>
                                            {if favorite_ids.get().contains(&item_fav.id) { i18n.t("dashboardActionUnfavorite") } else { i18n.t("dashboardActionFavorite") }}
                                        </button>
                                        <button type="button" data-testid="workspace-rename" on:click=move |_| {
                                            rename_name.set(item_rename.name.clone());
                                            rename_target.set(Some(item_rename.clone()));
                                            menu_id.set(None);
                                        }>{move || i18n.t("dashboardActionRename")}</button>
                                        <button type="button" data-testid="workspace-delete" on:click=move |_| {
                                            delete_target.set(Some(item_delete.clone()));
                                            menu_id.set(None);
                                        }>{move || i18n.t("dashboardActionDelete")}</button>
                                    </div>
                                </article>
                            }
                        }
                    />
                </div>
            </main>

            <AppDialog
                open=Signal::derive(move || rename_target.get().is_some())
                title_key="dashboardRenameDialogTitle"
                test_id="rename-workspace-dialog"
                on_close=Callback::new(move |_| rename_target.set(None))
            >
                <input data-testid="rename-workspace-input" prop:value=move || rename_name.get() on:input=move |ev| rename_name.set(event_target_value(&ev))/>
                <button type="button" data-testid="rename-workspace-confirm" on:click=move |_| {
                    if let Some(ws) = rename_target.get() {
                        apply_rename(token, workspaces, toaster, ws.id, rename_name.get(), ws.description.clone());
                    }
                    rename_target.set(None);
                }>{move || i18n.t("dashboardRenameSubmit")}</button>
            </AppDialog>
            <AppDialog
                open=Signal::derive(move || delete_target.get().is_some())
                title_key="dashboardDeleteDialogTitle"
                test_id="delete-workspace-dialog"
                on_close=Callback::new(move |_| delete_target.set(None))
            >
                <p>{move || i18n.t("dashboard.irreversible")}</p>
                <button type="button" data-testid="delete-workspace-confirm" on:click=move |_| {
                    if let Some(ws) = delete_target.get() {
                        apply_delete(token, workspaces, toaster, ws.id);
                    }
                    delete_target.set(None);
                }>{move || i18n.t("commonConfirmDelete")}</button>
            </AppDialog>
        </div>
        </ProductChrome>
    }
}

fn visible_workspaces(
    mut list: Vec<Workspace>,
    tab: String,
    favorites: Vec<String>,
    search: String,
    sort_mode: String,
) -> Vec<Workspace> {
    let q = search.trim().to_lowercase();
    list.retain(|ws| {
        let matches_search = q.is_empty() || ws.name.to_lowercase().contains(&q);
        let matches_tab = match tab.as_str() {
            "favorites" => favorites.contains(&ws.id),
            _ => true,
        };
        matches_search && matches_tab
    });
    if sort_mode == "name" {
        list.sort_by(|a, b| a.name.cmp(&b.name));
    } else {
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }
    list
}

fn current_token(token: RwSignal<String>) -> String {
    if token.get_untracked().is_empty() {
        web_sdk::read_browser_auth().map(|a| a.token).unwrap_or_default()
    } else {
        token.get_untracked()
    }
}

fn toggle_favorite(
    token: RwSignal<String>,
    favorite_ids: RwSignal<Vec<String>>,
    toaster: Toaster,
    workspace_id: String,
) {
    let tok = current_token(token);
    if tok.is_empty() {
        return;
    }
    let mut next = favorite_ids.get_untracked();
    if next.contains(&workspace_id) {
        next.retain(|id| id != &workspace_id);
    } else {
        next.push(workspace_id);
    }
    favorite_ids.set(next.clone());
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
        let mut prefs = client.get_preferences().await.unwrap_or_default();
        prefs.dashboard.favorite_workspace_ids = next;
        if client.put_preferences(&prefs).await.is_ok() {
            toaster.push(t_now("dashboard.favoriteUpdated"));
        }
    });
}

fn apply_rename(
    token: RwSignal<String>,
    workspaces: RwSignal<Vec<Workspace>>,
    toaster: Toaster,
    workspace_id: String,
    name: String,
    description: String,
) {
    let tok = current_token(token);
    if tok.is_empty() || name.trim().is_empty() {
        return;
    }
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
        if let Ok(resp) = client
            .update_workspace(&workspace_id, name.trim(), &description)
            .await
        {
            workspaces.update(|list| {
                if let Some(item) = list.iter_mut().find(|ws| ws.id == workspace_id) {
                    *item = resp.workspace;
                }
            });
            toaster.push(t_now("dashboard.renamed"));
        }
    });
}

fn apply_delete(
    token: RwSignal<String>,
    workspaces: RwSignal<Vec<Workspace>>,
    toaster: Toaster,
    workspace_id: String,
) {
    let tok = current_token(token);
    if tok.is_empty() {
        return;
    }
    leptos::task::spawn_local(async move {
        let client = BrowserRestClient::new(&poc_api_base(), Some(tok));
        if client.delete_workspace(&workspace_id).await.is_ok() {
            workspaces.update(|list| list.retain(|ws| ws.id != workspace_id));
            toaster.push(t_now("dashboard.deleted"));
        }
    });
}
