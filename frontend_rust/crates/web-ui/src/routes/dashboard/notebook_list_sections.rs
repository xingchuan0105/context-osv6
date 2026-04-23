fn notebook_list_sections(
    locale: ReadSignal<crate::i18n::Locale>,
    notebooks: Vec<Notebook>,
    workspace_href_base: String,
    current_user_id: String,
    favorite_notebook_ids: Vec<String>,
    toggle_notebook_favorite: StoredValue<Arc<dyn Fn(String) + Send + Sync + 'static>>,
    rename_notebook: StoredValue<Arc<dyn Fn(String, String, String) + Send + Sync + 'static>>,
    delete_notebook: StoredValue<Arc<dyn Fn(String) + Send + Sync + 'static>>,
) -> impl IntoView {
    let (open_menu_id, set_open_menu_id) = signal(Option::<String>::None);

    let rows = notebooks
        .into_iter()
        .map(|notebook| {
            let current_locale = locale.get();
            let notebook_id = notebook.id.clone();
            let notebook_id_for_favorite = StoredValue::new(notebook_id.clone());
            let notebook_id_for_rename = StoredValue::new(notebook_id.clone());
            let notebook_id_for_delete = StoredValue::new(notebook_id.clone());
            let notebook_id_for_menu_toggle = notebook_id.clone();
            let notebook_id_for_menu_visibility = notebook_id.clone();
            let notebook_title = dashboard_workspace_display_title(&notebook);
            let notebook_description_for_rename = StoredValue::new(notebook.description.clone());
            let notebook_title_for_rename = StoredValue::new(notebook_title.clone());
            let notebook_description =
                dashboard_notebook_description_label(current_locale, &notebook);
            let notebook_status_summary =
                dashboard_notebook_status_summary(current_locale, &notebook);
            let notebook_status_summary_for_show = notebook_status_summary.clone();
            let notebook_date = dashboard_notebook_date_label(current_locale, &notebook.updated_at);
            let role_label =
                dashboard_notebook_role_label(current_locale, notebook.owner_id == current_user_id);
            let source_count = notebook.document_count;
            let is_shared = notebook.shared;
            let is_favorite = favorite_notebook_ids.iter().any(|item| item == &notebook.id);

            view! {
                <A
                    href={format!("{}/{}", workspace_href_base, notebook.id)}
                    attr:class=dashboard_style::row
                >
                    <div class=dashboard_style::row_title_column>
                        <div class=dashboard_style::row_title_wrap>
                            <div class=dashboard_style::row_title>
                                {notebook_title}
                            </div>
                            <div class=dashboard_style::row_subtitle>
                                {notebook_description}
                            </div>
                            <div class=dashboard_style::row_badge_row>
                                <Show when=move || is_favorite>
                                    <span class=dashboard_style::row_chip>
                                        {move || choose(locale.get(), "收藏", "Favorite")}
                                    </span>
                                </Show>
                                <Show when=move || is_shared>
                                    <span class=dashboard_style::row_chip>
                                        {move || choose(locale.get(), "已分享", "Shared")}
                                    </span>
                                </Show>
                                <Show when=move || !notebook_status_summary_for_show.is_empty()>
                                    <span class=dashboard_style::row_chip>
                                        {notebook_status_summary.clone()}
                                    </span>
                                </Show>
                            </div>
                        </div>
                    </div>

                    <div class=dashboard_style::row_meta>
                        {format!("{} {}", source_count, choose(locale.get(), "个来源", "sources"))}
                    </div>

                    <div class=dashboard_style::row_meta>
                        {notebook_date}
                    </div>

                    <div class=dashboard_style::row_actions>
                        <span class=dashboard_style::row_meta>{role_label}</span>

                        <div class=dashboard_style::menu_anchor>
                            <button
                                type="button"
                                class=dashboard_style::row_menu_button
                                on:click=move |ev| {
                                    ev.prevent_default();
                                    ev.stop_propagation();
                                    let menu_id = notebook_id_for_menu_toggle.clone();
                                    set_open_menu_id.update(|current| {
                                        if current.as_deref() == Some(menu_id.as_str()) {
                                            *current = None;
                                        } else {
                                            *current = Some(menu_id.clone());
                                        }
                                    });
                                }
                            >
                                <svg class=dashboard_style::row_menu_icon fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6h.01M12 12h.01M12 18h.01"/>
                                </svg>
                            </button>

                            <Show when=move || open_menu_id.get().as_deref() == Some(notebook_id_for_menu_visibility.as_str())>
                                <div class=format!("{} {}", dashboard_style::menu, dashboard_style::row_menu)>
                                    <button
                                        type="button"
                                        class=dashboard_style::menu_item
                                        on:click=move |ev| {
                                            ev.prevent_default();
                                            ev.stop_propagation();
                                            set_open_menu_id.set(None);
                                            let notebook_id = notebook_id_for_favorite
                                                .with_value(|id| id.clone());
                                            toggle_notebook_favorite
                                                .with_value(|cb| cb(notebook_id));
                                        }
                                    >
                                        {move || {
                                            if is_favorite {
                                                choose(locale.get(), "取消收藏", "Remove favorite")
                                            } else {
                                                choose(locale.get(), "加入收藏", "Add favorite")
                                            }
                                        }}
                                    </button>
                                    <button
                                        type="button"
                                        class=dashboard_style::menu_item
                                        on:click=move |ev| {
                                            ev.prevent_default();
                                            ev.stop_propagation();
                                            set_open_menu_id.set(None);
                                            let notebook_id = notebook_id_for_rename
                                                .with_value(|id| id.clone());
                                            let notebook_title = notebook_title_for_rename
                                                .with_value(|title| title.clone());
                                            let notebook_description = notebook_description_for_rename
                                                .with_value(|desc| desc.clone());
                                            rename_notebook.with_value(|cb| cb(
                                                notebook_id,
                                                notebook_title,
                                                notebook_description,
                                            ));
                                        }
                                    >
                                        {move || choose(locale.get(), "重命名", "Rename")}
                                    </button>
                                    <button
                                        type="button"
                                        class=format!("{} {}", dashboard_style::menu_item, dashboard_style::menu_item_danger)
                                        on:click=move |ev| {
                                            ev.prevent_default();
                                            ev.stop_propagation();
                                            set_open_menu_id.set(None);
                                            let notebook_id = notebook_id_for_delete
                                                .with_value(|id| id.clone());
                                            delete_notebook.with_value(|cb| cb(notebook_id));
                                        }
                                    >
                                        {move || choose(locale.get(), "删除", "Delete")}
                                    </button>
                                </div>
                            </Show>
                        </div>
                    </div>
                </A>
            }
        })
        .collect_view();

    view! {
        <Show when=move || open_menu_id.get().is_some()>
            <button
                type="button"
                class=dashboard_style::backdrop
                aria-label={move || choose(locale.get(), "关闭菜单", "Close menu")}
                on:click=move |_| set_open_menu_id.set(None)
            />
        </Show>
        {rows}
    }
}
