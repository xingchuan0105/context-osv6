#[component]
fn AppearanceSettings() -> impl IntoView {
    let ui_prefs = use_ui_prefs_state();
    let locale = ui_prefs.locale;
    let theme = ui_prefs.theme;

    let theme_card_class = move |candidate: Theme| {
        format!(
            "rounded-xl border p-4 text-left transition-colors {}",
            if theme.get() == candidate {
                "border-primary/40 bg-primary/5 shadow-sm"
            } else {
                "border-border bg-card hover:border-primary/20"
            }
        )
    };

    let locale_card_class = move |candidate: Locale| {
        format!(
            "rounded-xl border p-4 text-left transition-colors {}",
            if locale.get() == candidate {
                "border-primary/40 bg-primary/5 shadow-sm"
            } else {
                "border-border bg-card hover:border-primary/20"
            }
        )
    };

    view! {
        <div class="space-y-6">
            <div class="app-surface-card">
                <div class="mb-5">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "主题模式", "Theme")}
                    </h3>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {move || choose(locale.get(), "控制工作台和后台页面的明暗观感。", "Choose how the workspace and admin surfaces should look.")}
                    </p>
                </div>

                <div class="grid gap-3 md:grid-cols-3">
                    <button
                        type="button"
                        class={move || theme_card_class(Theme::System)}
                        on:click=move |_| ui_prefs.set_theme.set(Theme::System)
                    >
                        <div class="text-sm font-semibold text-card-foreground">
                            {move || choose(locale.get(), "跟随系统", "System")}
                        </div>
                        <div class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "自动匹配设备当前主题。", "Follow the current operating system preference.")}
                        </div>
                    </button>
                    <button
                        type="button"
                        class={move || theme_card_class(Theme::Light)}
                        on:click=move |_| ui_prefs.set_theme.set(Theme::Light)
                    >
                        <div class="text-sm font-semibold text-card-foreground">
                            {move || choose(locale.get(), "浅色", "Light")}
                        </div>
                        <div class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "适合白天和高亮环境。", "Keep surfaces bright and high-contrast.")}
                        </div>
                    </button>
                    <button
                        type="button"
                        class={move || theme_card_class(Theme::Dark)}
                        on:click=move |_| ui_prefs.set_theme.set(Theme::Dark)
                    >
                        <div class="text-sm font-semibold text-card-foreground">
                            {move || choose(locale.get(), "深色", "Dark")}
                        </div>
                        <div class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "降低夜间眩光，强调工作区层次。", "Reduce glare and emphasize layered work surfaces.")}
                        </div>
                    </button>
                </div>
            </div>

            <div class="app-surface-card">
                <div class="mb-5">
                    <h3 class="text-lg font-semibold text-card-foreground">
                        {move || choose(locale.get(), "界面语言", "Language")}
                    </h3>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {move || choose(locale.get(), "当前产品默认中文，但你可以切换到英文。", "Chinese is the current default, but you can switch the interface to English.")}
                    </p>
                </div>

                <div class="grid gap-3 md:grid-cols-2">
                    <button
                        type="button"
                        class={move || locale_card_class(Locale::ZhCn)}
                        on:click=move |_| ui_prefs.set_locale.set(Locale::ZhCn)
                    >
                        <div class="text-sm font-semibold text-card-foreground">{"简体中文"}</div>
                        <div class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "面向当前主要用户群的默认语言。", "The default language for the current primary audience.")}
                        </div>
                    </button>
                    <button
                        type="button"
                        class={move || locale_card_class(Locale::En)}
                        on:click=move |_| ui_prefs.set_locale.set(Locale::En)
                    >
                        <div class="text-sm font-semibold text-card-foreground">{"English"}</div>
                        <div class="mt-1 text-sm text-muted-foreground">
                            {move || choose(locale.get(), "适合跨团队协作或对外演示。", "Useful for cross-team collaboration and external walkthroughs.")}
                        </div>
                    </button>
                </div>

                <div class="app-inline-surface mt-5 text-sm text-muted-foreground">
                    {move || {
                        let theme_label = match theme.get() {
                            Theme::System => choose(locale.get(), "跟随系统", "System"),
                            Theme::Light => choose(locale.get(), "浅色", "Light"),
                            Theme::Dark => choose(locale.get(), "深色", "Dark"),
                        };
                        let locale_label = match locale.get() {
                            Locale::ZhCn => "简体中文",
                            Locale::En => "English",
                        };
                        format!(
                            "{} {} · {} {}",
                            choose(locale.get(), "当前主题：", "Theme:"),
                            theme_label,
                            choose(locale.get(), "当前语言：", "Language:"),
                            locale_label
                        )
                    }}
                </div>
            </div>
        </div>
    }
}
