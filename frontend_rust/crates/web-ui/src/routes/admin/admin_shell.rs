#[component]
pub fn AdminShell(children: Children) -> impl IntoView {
    let location = use_location();
    let locale = use_ui_prefs_state().locale;
    let is_active = move |prefixes: &'static [&'static str]| {
        let pathname = location.pathname.get();
        prefixes.iter().any(|prefix| {
            if *prefix == "/admin" {
                pathname == *prefix
            } else {
                pathname == *prefix || pathname.starts_with(&format!("{prefix}/"))
            }
        })
    };

    view! {
        <div class="flex min-h-screen bg-muted/40">
            {/* Sidebar */}
            <div class="w-64 bg-card border-r border-border flex flex-col">
                <div class="p-4 border-b border-border">
                    <h2 class="text-xl font-bold text-foreground">
                        {move || choose(locale.get(), "后台管理", "Admin")}
                    </h2>
                </div>
                <nav class="flex-1 p-4 space-y-1">
                    {ADMIN_NAV_ITEMS
                        .iter()
                        .map(|item| {
                            let href = item.href;
                            let prefixes = item.prefixes;
                            let icon = item.icon;
                            view! {
                                <A href=href attr:class=move || nav_link_class(is_active(prefixes))>
                                    <AdminNavIconView icon=icon />
                                    {move || admin_nav_label(locale.get(), href)}
                                </A>
                            }
                        })
                        .collect_view()}
                </nav>
            </div>

            {/* Main content */}
            <div class="flex-1 overflow-auto">
                <div class="p-8">
                    {children()}
                </div>
            </div>
        </div>
    }
}
