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

    let format_date = |iso_string: &str| -> String {
        if iso_string.len() >= 10 {
            iso_string[..10].to_string()
        } else {
            iso_string.to_string()
        }
    };

    let rows = notebooks
        .into_iter()
        .map(|notebook| {
            let notebook_id = notebook.id.clone();
            let notebook_id_for_favorite = StoredValue::new(notebook_id.clone());
            let notebook_id_for_rename = StoredValue::new(notebook_id.clone());
            let notebook_id_for_delete = StoredValue::new(notebook_id.clone());
            let notebook_id_for_menu_toggle = notebook_id.clone();
            let notebook_id_for_menu_visibility = notebook_id.clone();
            let notebook_title = if notebook.title.trim().is_empty() {
                notebook.name.clone()
            } else {
                notebook.title.clone()
            };
            let notebook_description_for_rename = StoredValue::new(notebook.description.clone());
            let notebook_title_for_rename = StoredValue::new(notebook_title.clone());
            let notebook_date = format_date(&notebook.created_at);
            let role_label = if notebook.owner_id == current_user_id {
                choose(locale.get(), "所有者", "Owner").to_string()
            } else {
                choose(locale.get(), "成员", "Member").to_string()
            };
            let source_count = notebook.document_count;
            let is_shared = notebook.shared;
            let is_favorite = favorite_notebook_ids.iter().any(|item| item == &notebook.id);

            view! {
                <A
                    href={format!("{}/{}", workspace_href_base, notebook.id)}
                    attr:class="group grid grid-cols-12 items-center gap-4 border-b border-border px-5 py-4 transition-colors hover:bg-muted/40"
                >
                    <div class="col-span-6 flex min-w-0 items-center gap-3 pr-2">
                        <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
                            <svg class="h-4.5 w-4.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7 3h7l5 5v13H7z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M14 3v5h5"/>
                            </svg>
                        </div>
                        <div class="min-w-0">
                            <div class="truncate text-[18px] font-medium text-foreground">
                                {notebook_title}
                            </div>
                            <Show when=move || is_favorite || is_shared>
                                <div class="mt-1 text-[12px] text-muted-foreground">
                                    {move || {
                                        if is_favorite && is_shared {
                                            choose(locale.get(), "收藏 · 共享", "Favorite · Shared")
                                        } else if is_favorite {
                                            choose(locale.get(), "收藏", "Favorite")
                                        } else {
                                            choose(locale.get(), "共享", "Shared")
                                        }
                                    }}
                                </div>
                            </Show>
                        </div>
                    </div>

                    <div class="col-span-2 text-[16px] text-muted-foreground">
                        {format!("{} {}", source_count, choose(locale.get(), "个来源", "sources"))}
                    </div>

                    <div class="col-span-2 text-[16px] text-muted-foreground">
                        {notebook_date}
                    </div>

                    <div class="col-span-2 flex items-center justify-between gap-2">
                        <span class="text-[16px] text-muted-foreground">{role_label}</span>

                        <div class="relative">
                            <button
                                type="button"
                                class="rounded-full p-2 text-muted-foreground opacity-0 transition-colors duration-150 hover:bg-muted hover:text-foreground group-hover:opacity-100"
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
                                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6h.01M12 12h.01M12 18h.01"/>
                                </svg>
                            </button>

                            <Show when=move || open_menu_id.get().as_deref() == Some(notebook_id_for_menu_visibility.as_str())>
                                <div class="absolute right-0 top-10 z-20 w-44 rounded-2xl border border-border bg-card p-1.5 shadow-lg">
                                    <button
                                        type="button"
                                        class="block w-full rounded-xl px-3 py-2 text-left text-[14px] text-foreground hover:bg-muted"
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
                                        class="block w-full rounded-xl px-3 py-2 text-left text-[14px] text-foreground hover:bg-muted"
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
                                        class="block w-full rounded-xl px-3 py-2 text-left text-[14px] text-destructive hover:bg-destructive/10"
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
                class="fixed inset-0 z-10 bg-transparent"
                aria-label={move || choose(locale.get(), "关闭菜单", "Close menu")}
                on:click=move |_| set_open_menu_id.set(None)
            />
        </Show>
        {rows}
    }
}
