#[component]
fn ShareCenterPanels(
    active_tab: ReadSignal<ShareTab>,
    locale: ReadSignal<crate::i18n::Locale>,
    settings: ReadSignal<Option<ShareSettings>>,
    analytics: ReadSignal<Option<ShareAnalyticsResponse>>,
    logs: ReadSignal<Option<AccessLogsResponse>>,
    members: ReadSignal<Vec<MemberRow>>,
    invite_email: ReadSignal<String>,
    set_invite_email: WriteSignal<String>,
    invite_role: ReadSignal<String>,
    set_invite_role: WriteSignal<String>,
    inviting: ReadSignal<bool>,
    on_settings_updated: Arc<dyn Fn(ShareSettings) + Send + Sync>,
    on_enable_toggle: Arc<dyn Fn(ShareSettings) + Send + Sync>,
    on_invite: Arc<dyn Fn() + Send + Sync>,
    set_remove_member_id: WriteSignal<Option<String>>,
) -> impl IntoView {
    let on_settings_updated_for_panel = StoredValue::new(on_settings_updated);
    let on_enable_toggle_for_panel = StoredValue::new(on_enable_toggle);
    let on_invite_for_panel = StoredValue::new(on_invite);
    let settings_for_panel = Signal::derive(move || settings.get());
    let analytics_for_panel = Signal::derive(move || analytics.get());
    let logs_for_panel = Signal::derive(move || logs.get());
    let members_for_panel = Signal::derive(move || members.get());

    view! {
        <Show when=move || active_tab.get() == ShareTab::Settings>
            <Show
                when=move || settings_for_panel.get().is_some()
                fallback=move || view! {
                    <div class=shared_page_style::loading_state>
                        {move || choose(locale.get(), "正在加载设置...", "Loading settings...")}
                    </div>
                }
            >
                {move || settings_for_panel.get().map(|settings| view! {
                    <ShareSettingsPanel
                        settings=settings
                        on_settings_updated=on_settings_updated_for_panel.get_value()
                        on_enable_toggle=on_enable_toggle_for_panel.get_value()
                    />
                    <MembersPanel
                        members={members_for_panel.get()}
                        invite_email=invite_email
                        set_invite_email=set_invite_email
                        invite_role=invite_role
                        set_invite_role=set_invite_role
                        inviting=inviting
                        on_invite=on_invite_for_panel.get_value()
                        set_remove_member_id=set_remove_member_id
                    />
                })}
            </Show>
        </Show>

        <Show when=move || active_tab.get() == ShareTab::Analytics>
            <Show
                when=move || analytics_for_panel.get().is_some()
                fallback=move || view! {
                    <div class=shared_page_style::loading_state>
                        {move || choose(locale.get(), "正在加载分析...", "Loading analytics...")}
                    </div>
                }
            >
                {move || analytics_for_panel.get().map(|analytics| view! {
                    <ShareAnalytics analytics=analytics />
                })}
            </Show>
        </Show>

        <Show when=move || active_tab.get() == ShareTab::AccessLogs>
            <Show
                when=move || logs_for_panel.get().is_some()
                fallback=move || view! {
                    <div class=shared_page_style::loading_state>
                        {move || choose(locale.get(), "正在加载访问日志...", "Loading access logs...")}
                    </div>
                }
            >
                {move || logs_for_panel.get().map(|logs| view! {
                    <ShareAccessLogs logs=logs.logs />
                })}
            </Show>
        </Show>
    }
}
