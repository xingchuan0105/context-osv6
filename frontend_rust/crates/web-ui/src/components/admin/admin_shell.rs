use crate::api_base::poc_api_base;
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use web_sdk::{BrowserRestClient, TransportError};

/// 管理后台子导航（13 端点中除账户详情外的全部目的地；入口闭环）。
pub const ADMIN_NAV_ITEMS: &[(&str, &str, &str)] = &[
    ("overview", "/admin", "概览"),
    ("accounts", "/admin/accounts", "账户管理"),
    ("users", "/admin/users", "用户管理"),
    ("usage", "/admin/usage", "用量监控"),
    ("billing", "/admin/billing", "平台计费"),
    ("health", "/admin/health", "服务健康"),
    ("rag-health", "/admin/rag-health", "RAG 健康"),
    ("workers", "/admin/system/workers", "Workers"),
    ("degradation", "/admin/system/degradation", "降级策略"),
    ("broadcast", "/admin/broadcast", "公告广播"),
    ("audit-logs", "/admin/audit-logs", "审计日志"),
    ("feature-flags", "/admin/feature-flags", "功能开关"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminGateState {
    Loading,
    Ready,
    LoginRequired,
    Forbidden,
}

/// 每个管理页面共享的门禁与错误状态；页面自身创建并 provide，AdminShell 集中渲染。
#[derive(Clone, Copy)]
pub struct AdminPageState {
    pub gate: RwSignal<AdminGateState>,
    pub error: RwSignal<Option<String>>,
}

impl AdminPageState {
    pub fn new() -> Self {
        Self {
            gate: RwSignal::new(AdminGateState::Loading),
            error: RwSignal::new(None),
        }
    }

    pub fn set_loading(&self) {
        self.gate.set(AdminGateState::Loading);
    }

    pub fn set_ready(&self) {
        self.gate.set(AdminGateState::Ready);
    }

    pub fn set_error(&self, message: impl Into<String>) {
        self.gate.set(AdminGateState::Ready);
        self.error.set(Some(message.into()));
    }

    /// 401 → 登录跳转；403 → 无权访问面板；其余 → 页面内错误提示。
    pub fn report_error(&self, err: &TransportError) {
        match err {
            TransportError::Unauthorized => self.gate.set(AdminGateState::LoginRequired),
            TransportError::Forbidden(_) => self.gate.set(AdminGateState::Forbidden),
            other => self.set_error(format!("加载失败：{other}")),
        }
    }
}

pub fn resolve_admin_token(token: &RwSignal<String>) -> String {
    let tok = token.get_untracked();
    if !tok.is_empty() {
        return tok;
    }
    web_sdk::read_browser_auth()
        .map(|auth| {
            token.set(auth.token.clone());
            auth.token
        })
        .unwrap_or_default()
}

pub fn admin_client(token: &str) -> BrowserRestClient {
    BrowserRestClient::new(&poc_api_base(), Some(token.to_string()))
}

fn admin_path_is_active(pathname: &str, href: &str) -> bool {
    if href == "/admin" {
        pathname == href
    } else {
        pathname == href || pathname.starts_with(&format!("{href}/"))
    }
}

#[component]
pub fn AdminShell(
    title: &'static str,
    test_id: &'static str,
    children: ChildrenFn,
) -> impl IntoView {
    let token = expect_context::<RwSignal<String>>();
    let navigate = use_navigate();
    let location = use_location();
    let pathname = location.pathname;
    let state = expect_context::<AdminPageState>();

    let shell_state = state.clone();
    Effect::new(move |_| {
        let mut tok = token.get();
        if tok.is_empty() {
            if let Some(auth) = web_sdk::read_browser_auth() {
                tok = auth.token.clone();
                token.set(auth.token);
            }
        }
        if tok.is_empty() {
            shell_state.gate.set(AdminGateState::LoginRequired);
        }
    });

    let redirect_state = state.clone();
    Effect::new(move |_| {
        if redirect_state.gate.get() == AdminGateState::LoginRequired {
            let next = pathname.get_untracked();
            navigate(&format!("/login?next={next}"), NavigateOptions::default());
        }
    });

    let login_href = Signal::derive(move || format!("/login?next={}", pathname.get()));

    view! {
        <div class="admin-shell" data-testid=test_id>
            <header class="admin-header">
                <div class="admin-header-left">
                    <a href="/dashboard" class="admin-back-link">"← 返回工作台"</a>
                    <h1 class="admin-title">{title}</h1>
                </div>
            </header>

            <nav class="admin-nav" aria-label="管理后台导航">
                {ADMIN_NAV_ITEMS
                    .iter()
                    .map(|(id, href, label)| {
                        let href = *href;
                        view! {
                            <a
                                href=href
                                class=move || {
                                    if admin_path_is_active(&pathname.get(), href) {
                                        "admin-nav-item is-active"
                                    } else {
                                        "admin-nav-item"
                                    }
                                }
                                data-testid=format!("admin-nav-{id}")
                            >
                                {*label}
                            </a>
                        }
                    })
                    .collect_view()}
            </nav>

            <main class="admin-content">
                {move || {
                    state.error.get().map(|msg| {
                        view! { <p class="admin-error" role="alert" data-testid="admin-error">{msg}</p> }
                    })
                }}
                {move || match state.gate.get() {
                    AdminGateState::Ready => children().into_any(),
                    AdminGateState::Loading => {
                        view! {
                            <p class="admin-loading" data-testid="admin-loading">
                                "正在校验访问权限…"
                            </p>
                        }
                            .into_any()
                    }
                    AdminGateState::LoginRequired => {
                        view! {
                            <section class="admin-panel" data-testid="admin-login-required">
                                <p class="admin-empty-text">"管理员功能需要登录后访问。"</p>
                                <a href=login_href class="admin-panel-btn" data-testid="admin-login-link">
                                    "前往登录"
                                </a>
                            </section>
                        }
                            .into_any()
                    }
                    AdminGateState::Forbidden => {
                        view! {
                            <section
                                class="admin-panel admin-forbidden"
                                role="alert"
                                data-testid="admin-forbidden"
                            >
                                <p class="admin-empty-text">
                                    "当前账号无权访问管理后台。该区域仅限平台管理员使用。"
                                </p>
                                <a href="/dashboard" class="admin-panel-btn">"返回工作台"</a>
                            </section>
                        }
                            .into_any()
                    }
                }}
            </main>
        </div>
    }
}
