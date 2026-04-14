fn notebook_card_sections(
    locale: ReadSignal<crate::i18n::Locale>,
    notebooks: Vec<Notebook>,
    workspace_href_base: String,
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

    let cards = notebooks
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
            let notebook_title_for_rename = StoredValue::new(notebook_title.clone());
            let notebook_description_for_rename = StoredValue::new(notebook.description.clone());
            let notebook_date = format_date(&notebook.updated_at);
            let icon_label = notebook_title
                .chars()
                .next()
                .map(|ch| ch.to_string())
                .unwrap_or_else(|| "📘".to_string());
            let is_shared = notebook.shared;
            let is_favorite = favorite_notebook_ids
                .iter()
                .any(|item| item == &notebook.id);
            let source_count = notebook.document_count;
            let access_label = if is_shared {
                choose(locale.get(), "共享", "Shared").to_string()
            } else {
                choose(locale.get(), "私有", "Private").to_string()
            };

            view! {
                <A
                    href={format!("{}/{}", workspace_href_base, notebook.id)}
                    attr:class="group relative block h-[188px] rounded-2xl border border-border/80 bg-card px-5 py-4 transition-all duration-150 hover:border-border hover:shadow-md"
                >
                    <div class="flex items-start justify-between gap-3">
                        <div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-muted/80 text-base font-semibold text-foreground">
                            {icon_label}
                        </div>

                        <div class="relative">
                            <button
                                type="button"
                                class="rounded-full p-1.5 text-muted-foreground opacity-0 transition-colors duration-150 hover:bg-muted hover:text-foreground group-hover:opacity-100"
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
                                <div class="absolute right-0 top-9 z-20 w-44 rounded-xl border border-border bg-card p-1 shadow-lg">
                                    <button
                                        type="button"
                                        class="block w-full rounded-lg px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
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
                                        class="block w-full rounded-lg px-3 py-2 text-left text-sm text-foreground hover:bg-muted"
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
                                        class="block w-full rounded-lg px-3 py-2 text-left text-sm text-destructive hover:bg-destructive/10"
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

                    <div class="mt-4 min-w-0">
                        <h3 class="line-clamp-2 text-[15px] font-medium leading-snug text-foreground">
                            {notebook_title}
                        </h3>
                    </div>

                    <div class="mt-3 flex flex-wrap items-center gap-1.5">
                        <Show when=move || is_favorite>
                            <span class="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
                                {move || choose(locale.get(), "收藏", "Favorite")}
                            </span>
                        </Show>
                        <Show when=move || is_shared>
                            <span class="rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
                                {move || choose(locale.get(), "共享", "Shared")}
                            </span>
                        </Show>
                        <Show when=move || { source_count > 0 }>
                            <span class="rounded-full bg-muted px-2 py-0.5 text-[10px] text-muted-foreground">
                                {format!("{} {}", source_count, choose(locale.get(), "个来源", "sources"))}
                            </span>
                        </Show>
                    </div>

                    <div class="mt-3 flex items-center justify-between text-[12px] text-muted-foreground">
                        <span>{notebook_date}</span>
                        <span class="inline-flex items-center gap-1">
                            <Show when=move || !is_shared>
                                <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 11c1.657 0 3-1.343 3-3V6a3 3 0 10-6 0v2c0 1.657 1.343 3 3 3zm-7 9h14a2 2 0 002-2v-5a2 2 0 00-2-2H5a2 2 0 00-2 2v5a2 2 0 002 2z"/>
                                </svg>
                            </Show>
                            <span>{access_label}</span>
                        </span>
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
        {cards}
    }
}
