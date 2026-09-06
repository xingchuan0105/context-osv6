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
    DesktopBuyPage, EnPricingPage, PaywallPage, PricingPage, UpgradeSuccessPage,
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
use leptos::prelude::*;
use leptos_config::LeptosOptions;
use leptos_meta::{HashedStylesheet, MetaTags, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    provide_context(RwSignal::new(ChatCanvasModel::new()));
    provide_context(RwSignal::new(String::new()));

    view! {
        <AuthBootstrap/>
        <Router>
            <Routes fallback=|| {
                view! {
                    <main class="chat-not-found">
                        <p>"页面不存在。"</p>
                        <a href="/chat">"返回对话"</a>
                    </main>
                }
            }>
                <Route path=path!("/") view=HomePage/>
                <Route path=path!("/chat/:session_id?") view=ChatPage/>
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
                <Route path=path!("/login") view=LoginPage/>
                <Route path=path!("/register") view=RegisterPage/>
                <Route path=path!("/reset-password") view=ResetPasswordRequestPage/>
                <Route path=path!("/reset-password/verify") view=ResetPasswordVerifyPage/>
                <Route path=path!("/reset-password/confirm") view=ResetPasswordConfirmPage/>
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
                <Route path=path!("/help/faq") view=HelpFaqPage/>
                <Route path=path!("/help/compare") view=HelpComparePage/>
                <Route path=path!("/help/api-access") view=HelpApiAccessPage/>
                <Route path=path!("/help/api-access/agents") view=HelpAgentApiPage/>
                <Route path=path!("/integrations") view=IntegrationIndexPage/>
                <Route path=path!("/integrations/mcp") view=IntegrationMcpPage/>
                <Route path=path!("/integrations/claude-desktop") view=IntegrationClaudeDesktopPage/>
                <Route path=path!("/integrations/cursor") view=IntegrationCursorPage/>
                <Route path=path!("/desktop") view=DesktopPage/>
                <Route path=path!("/activate") view=DesktopActivatePage/>
                <Route path=path!("/setup") view=DesktopSetupPage/>
                <Route path=path!("/legal") view=LegalCenterPage/>
                <Route path=path!("/legal/terms") view=LegalTermsPage/>
                <Route path=path!("/legal/privacy") view=LegalPrivacyPage/>
                <Route path=path!("/legal/licenses") view=LegalLicensesPage/>
                <Route path=path!("/legal/licenses/project") view=LegalProjectLicensePage/>
                <Route path=path!("/legal/licenses/third-party") view=LegalThirdPartyPage/>
                <Route path=path!("/en") view=EnHomePage/>
                <Route path=path!("/en/pricing") view=EnPricingPage/>
                <Route path=path!("/en/desktop") view=EnDesktopPage/>
                <Route path=path!("/en/help/faq") view=EnHelpFaqPage/>
                <Route path=path!("/en/help/compare") view=EnHelpComparePage/>
                <Route path=path!("/en/help/api-access") view=EnHelpApiAccessPage/>
                <Route path=path!("/en/help/api-access/agents") view=EnHelpAgentApiPage/>
                <Route path=path!("/en/legal") view=EnLegalCenterPage/>
                <Route path=path!("/en/legal/terms") view=EnLegalTermsPage/>
                <Route path=path!("/en/legal/privacy") view=EnLegalPrivacyPage/>
                <Route path=path!("/en/legal/licenses") view=EnLegalLicensesPage/>
                <Route path=path!("/en/legal/licenses/project") view=EnLegalProjectLicensePage/>
                <Route path=path!("/en/legal/licenses/third-party") view=EnLegalThirdPartyPage/>
                <Route path=path!("/pricing") view=PricingPage/>
                <Route path=path!("/upgrade/paywall") view=PaywallPage/>
                <Route path=path!("/upgrade/success") view=UpgradeSuccessPage/>
                <Route path=path!("/desktop/buy") view=DesktopBuyPage/>
            </Routes>
        </Router>
    }
}

/// SSR HTML 外壳：包含可读的页面骨架与表单语义（由 App SSR 输出），
/// 以及 hydration 脚本与样式链接。
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>"Context-OS Chat"</title>
                <HashedStylesheet id="leptos" options=options.clone()/>
                <link rel="stylesheet" href="/style/chat-poc.css"/>
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
