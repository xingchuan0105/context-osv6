use crate::auth::AuthBootstrap;
use crate::components::auth::{
    LoginPage, RegisterPage, ResetPasswordConfirmPage, ResetPasswordRequestPage,
    ResetPasswordVerifyPage,
};
use crate::components::chat::ChatCanvasModel;
use crate::components::chat::chat_page::ChatPage;
use crate::components::settings::{SettingsPage, UsagePage};
use leptos::prelude::*;
use leptos_config::LeptosOptions;
use leptos_meta::{HashedStylesheet, MetaTags, provide_meta_context};
use leptos_router::components::{Redirect, Route, Router, Routes};
use leptos_router::path;

#[component]
fn RedirectToChat() -> impl IntoView {
    view! { <Redirect path="/chat"/> }
}

/// 根组件：App 级提供唯一的 ChatCanvasModel 信号上下文。
/// 客户端导航（/chat → /chat/:id）不重建该模型，保证流不被 URL 落地打断。
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
                <Route path=path!("/") view=RedirectToChat/>
                <Route path=path!("/chat/:session_id?") view=ChatPage/>
                <Route path=path!("/dashboard/:workspace_id?") view=ChatPage/>
                <Route path=path!("/login") view=LoginPage/>
                <Route path=path!("/register") view=RegisterPage/>
                <Route path=path!("/reset-password") view=ResetPasswordRequestPage/>
                <Route path=path!("/reset-password/verify") view=ResetPasswordVerifyPage/>
                <Route path=path!("/reset-password/confirm") view=ResetPasswordConfirmPage/>
                <Route path=path!("/settings") view=SettingsPage/>
                <Route path=path!("/settings/usage") view=UsagePage/>
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
