use crate::auth::AuthBootstrap;
use crate::components::admin::{
    AdminAccountDetailPage, AdminAccountsPage, AdminAuditLogsPage, AdminBillingPage,
    AdminBroadcastPage, AdminDegradationPage, AdminFeatureFlagsPage, AdminHealthPage,
    AdminOverviewPage, AdminRagHealthPage, AdminUsagePage, AdminUsersPage, AdminWorkersPage,
};
use crate::components::auth::{
    LoginPage, RegisterPage, ResetPasswordConfirmPage, ResetPasswordRequestPage,
    ResetPasswordVerifyPage,
};
use crate::components::billing::{
    DesktopBuyPage, EnPricingPage, PaywallPage, UpgradeSuccessPage, ZhPricingPage,
};
use crate::components::chat::ChatCanvasModel;
use crate::components::chat::chat_page::ChatPage;
use crate::components::dashboard::{
    DashboardOverviewPage, GlobalAnalyticsPage, WorkspaceAnalyzePage, WorkspaceWorkbenchPage,
};
use crate::components::help::{
    EnHelpApiAccessPage, EnHelpAgentApiPage, EnHelpComparePage, EnHelpFaqPage, HelpApiAccessPage,
    HelpAgentApiPage, HelpComparePage, HelpFaqPage, HelpPage, HelpWritePage,
};
use crate::components::integrations::{
    IntegrationClaudeDesktopPage, IntegrationCursorPage, IntegrationIndexPage, IntegrationMcpPage,
};
use crate::components::legal::{
    EnLegalCenterPage, EnLegalLicensesPage, EnLegalPrivacyPage, EnLegalProjectLicensePage,
    EnLegalTermsPage, EnLegalThirdPartyPage, LegalCenterPage, LegalLicensesPage, LegalPrivacyPage,
    LegalProjectLicensePage, LegalTermsPage, LegalThirdPartyPage,
};
use crate::components::marketing::{
    DesktopActivatePage, DesktopPage, DesktopSetupPage, EnDesktopPage, EnHomePage, HomePage,
};
use crate::components::settings::{SettingsPage, UsagePage};
use crate::components::share::{
    InvitePage, SharedKbPage, SharedUserPage, WorkspaceShareAnalyticsPage,
    WorkspaceShareLogsPage, WorkspaceSharePage,
};
use crate::components::ui::{ToastHost, provide_toaster};
use crate::i18n::{Tx, provide_i18n};
use leptos::prelude::*;
use leptos_config::LeptosOptions;
use leptos_meta::{HashedStylesheet, MetaTags, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_i18n();
    provide_context(RwSignal::new(ChatCanvasModel::new()));
    provide_context(crate::components::chat::chat_page::SharedChatModel(RwSignal::new(ChatCanvasModel::new())));
    provide_context(RwSignal::new(String::new()));
    provide_toaster();

    view! {
        <leptos_meta::Title text="Context-OS"/>
        <AuthBootstrap/>
        <ToastHost/>
        <Router>
            <Routes fallback=|| {
                view! {
                    <main class="chat-not-found">
                        <p><Tx k="notFound.title"/></p>
                        <a href="/chat"><Tx k="notFound.backToChat"/></a>
                    </main>
                }
            }>
                <Route path=path!("/") view=|| view! { <crate::components::shell::public_layout::PublicLayout><HomePage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/chat/:session_id?") view=|| view! { <ChatPage/> }/>
                <Route path=path!("/dashboard") view=DashboardOverviewPage/>
                <Route path=path!("/dashboard/analytics") view=GlobalAnalyticsPage/>
                <Route path=path!("/dashboard/:workspace_id/analyze") view=WorkspaceAnalyzePage/>
                <Route path=path!("/dashboard/:workspace_id/share/access-logs") view=WorkspaceShareLogsPage/>
                <Route path=path!("/dashboard/:workspace_id/share/analytics") view=WorkspaceShareAnalyticsPage/>
                <Route path=path!("/dashboard/:workspace_id/share") view=WorkspaceSharePage/>
                <Route path=path!("/dashboard/:workspace_id") view=WorkspaceWorkbenchPage/>
                <Route path=path!("/shared/kb/:token") view=SharedKbPage/>
                <Route path=path!("/shared/u/:user_id") view=SharedUserPage/>
                <Route path=path!("/invite/:workspace_id/:member_id") view=InvitePage/>
                <Route path=path!("/login") view=|| view! { <crate::components::shell::public_layout::PublicLayout><LoginPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/register") view=|| view! { <crate::components::shell::public_layout::PublicLayout><RegisterPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/reset-password") view=|| view! { <crate::components::shell::public_layout::PublicLayout><ResetPasswordRequestPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/reset-password/verify") view=|| view! { <crate::components::shell::public_layout::PublicLayout><ResetPasswordVerifyPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/reset-password/confirm") view=|| view! { <crate::components::shell::public_layout::PublicLayout><ResetPasswordConfirmPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/settings") view=SettingsPage/>
                <Route path=path!("/settings/usage") view=UsagePage/>
                <Route path=path!("/admin") view=AdminOverviewPage/>
                <Route path=path!("/admin/accounts") view=AdminAccountsPage/>
                <Route path=path!("/admin/accounts/:owner_user_id") view=AdminAccountDetailPage/>
                <Route path=path!("/admin/users") view=AdminUsersPage/>
                <Route path=path!("/admin/usage") view=AdminUsagePage/>
                <Route path=path!("/admin/billing") view=AdminBillingPage/>
                <Route path=path!("/admin/health") view=AdminHealthPage/>
                <Route path=path!("/admin/rag-health") view=AdminRagHealthPage/>
                <Route path=path!("/admin/system/workers") view=AdminWorkersPage/>
                <Route path=path!("/admin/system/degradation") view=AdminDegradationPage/>
                <Route path=path!("/admin/broadcast") view=AdminBroadcastPage/>
                <Route path=path!("/admin/audit-logs") view=AdminAuditLogsPage/>
                <Route path=path!("/admin/feature-flags") view=AdminFeatureFlagsPage/>
                <Route path=path!("/help") view=HelpPage/>
                <Route path=path!("/help/write") view=HelpWritePage/>
                <Route path=path!("/help/faq") view=|| view! { <crate::components::shell::public_layout::PublicLayout><HelpFaqPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/help/compare") view=|| view! { <crate::components::shell::public_layout::PublicLayout><HelpComparePage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/help/api-access") view=|| view! { <crate::components::shell::public_layout::PublicLayout><HelpApiAccessPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/help/api-access/agents") view=|| view! { <crate::components::shell::public_layout::PublicLayout><HelpAgentApiPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/integrations") view=|| view! { <crate::components::shell::public_layout::PublicLayout><IntegrationIndexPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/integrations/mcp") view=|| view! { <crate::components::shell::public_layout::PublicLayout><IntegrationMcpPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/integrations/claude-desktop") view=|| view! { <crate::components::shell::public_layout::PublicLayout><IntegrationClaudeDesktopPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/integrations/cursor") view=|| view! { <crate::components::shell::public_layout::PublicLayout><IntegrationCursorPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/desktop") view=DesktopPage/>
                <Route path=path!("/activate") view=|| view! { <crate::components::shell::public_layout::PublicLayout><DesktopActivatePage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/setup") view=|| view! { <crate::components::shell::public_layout::PublicLayout><DesktopSetupPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/legal") view=LegalCenterPage/>
                <Route path=path!("/legal/terms") view=LegalTermsPage/>
                <Route path=path!("/legal/privacy") view=LegalPrivacyPage/>
                <Route path=path!("/legal/licenses") view=LegalLicensesPage/>
                <Route path=path!("/legal/licenses/project") view=LegalProjectLicensePage/>
                <Route path=path!("/legal/licenses/third-party") view=LegalThirdPartyPage/>
                <Route path=path!("/en") view=|| view! { <crate::components::shell::public_layout::PublicLayout locale="en"><EnHomePage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/en/pricing") view=EnPricingPage/>
                <Route path=path!("/en/desktop") view=EnDesktopPage/>
                <Route path=path!("/en/help/faq") view=|| view! { <crate::components::shell::public_layout::PublicLayout locale="en"><EnHelpFaqPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/en/help/compare") view=|| view! { <crate::components::shell::public_layout::PublicLayout locale="en"><EnHelpComparePage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/en/help/api-access") view=|| view! { <crate::components::shell::public_layout::PublicLayout locale="en"><EnHelpApiAccessPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/en/help/api-access/agents") view=|| view! { <crate::components::shell::public_layout::PublicLayout locale="en"><EnHelpAgentApiPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/en/legal") view=EnLegalCenterPage/>
                <Route path=path!("/en/legal/terms") view=EnLegalTermsPage/>
                <Route path=path!("/en/legal/privacy") view=EnLegalPrivacyPage/>
                <Route path=path!("/en/legal/licenses") view=EnLegalLicensesPage/>
                <Route path=path!("/en/legal/licenses/project") view=EnLegalProjectLicensePage/>
                <Route path=path!("/en/legal/licenses/third-party") view=EnLegalThirdPartyPage/>
                <Route path=path!("/pricing") view=ZhPricingPage/>
                <Route path=path!("/upgrade/paywall") view=|| view! { <crate::components::shell::public_layout::PublicLayout><PaywallPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/upgrade/success") view=|| view! { <crate::components::shell::public_layout::PublicLayout><UpgradeSuccessPage/></crate::components::shell::public_layout::PublicLayout> }/>
                <Route path=path!("/desktop/buy") view=|| view! { <crate::components::shell::public_layout::PublicLayout><DesktopBuyPage/></crate::components::shell::public_layout::PublicLayout> }/>
            </Routes>
        </Router>
    }
}

/// SSR HTML 外壳：包含可读的页面骨架与表单语义（由 App SSR 输出），
/// 以及 hydration 脚本与样式链接。
pub fn shell(options: LeptosOptions) -> impl IntoView {
    let turnstile_site_key = std::env::var("NEXT_PUBLIC_TURNSTILE_SITE_KEY").unwrap_or_default();
    view! {
        <!DOCTYPE html>
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="turnstile-site-key" content=turnstile_site_key/>
                <HashedStylesheet id="leptos" options=options.clone()/>
                <link rel="stylesheet" href="/style/app.css"/>
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}
